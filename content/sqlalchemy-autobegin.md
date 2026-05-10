+++
title = "Good intentions, confusing autobegin behavior"
date = 2025-08-08
+++

## Inheriting code-bases

Software engineers come and go, but code remains. When coworkers leave, you inherit their code and may even need to add to it. You learn a lot about how they think and inevitably adopt their mindset in an effort to deconstruct their logic.

After inheriting a previous software engineer's service and needing to add a feature, my coworker had identified an interesting error in the unit test suite. The error appeared intermittently during database operations when test cleanup was attempted.

## A puzzling error

The error:

```python
self = <sqlalchemy.orm.session.SessionTransaction object at 0x7fa132ff1d00>
operation_name = '_begin', state = <SessionTransactionState.DEACTIVE: 4>

    def _raise_for_prerequisite_state(
        self, operation_name: str, state: _StateChangeState
    ) -> NoReturn:
        if state is SessionTransactionState.DEACTIVE:
            if self._rollback_exception:
                raise sa_exc.PendingRollbackError(
                    "This Session's transaction has been rolled back "
                    "due to a previous exception during flush."
                    " To begin a new transaction with this Session, "
                    "first issue Session.rollback()."
                    f" Original exception was: {self._rollback_exception}",
                    code="7s2a",
                )
            else:
>               raise sa_exc.InvalidRequestError(
                    "This session is in 'inactive' state, due to the "
                    "SQL transaction being rolled back; no further SQL "
                    "can be emitted within this transaction."
E                   sqlalchemy.exc.InvalidRequestError: This session is in 'inactive' state, due to the SQL transaction being rolled back; no further SQL can be emitted within this transaction.
```

That exception was thrown at this point in a `conftest.py` fixture whose aim was to yield a patched database session and perform a rollback as a clean up step:

```python
db_conn = <sqlalchemy.engine.base.Connection object at 0x7fa132ec4b20>

    @pytest.fixture
    def db_session(db_conn):
        session = Session(
            bind=db_conn,
            autocommit=False,
            autoflush=False,
            join_transaction_mode="create_savepoint",
        )
        tx = session.begin()
    
        patched_session = _test_patch_db_session(tx)
        yield patched_session
        try:
            # sanity tests for _test_patch_db_session ensuring our code
            # and this set of workarounds are behaving properly
            assert tx.is_active, "Top-level test TX is no longer active"
        finally:
            if tx.is_active:
>               tx.rollback()
```

What was weird was that the error occurred during fixture cleanup, meaning perhaps something was happening during the test that corrupted the session state, preventing normal cleanup.

## The context: realistic testing patterns

While examining the `_test_patch_db_session` function, I encountered something I hadn't seen before - a patch to make database tests behave more like production code.

In production, our endpoints create explicit transactions for each request:

```python
def delete_example(
    db: DbDep,
    id: Annotated[PositiveInt, Path()],
):
    with db.begin():
        user = crud.delete_example(db)
    if not user:
        raise HTTPException(status_code=status.HTTP_404_NOT_FOUND)
    audit.log(f"Deleted id {id} impacting user {user}")
```

`crud.delete_example` here would just use `db.execute` without calling an explicit begin, and in the tests we test `crud.delete_example` directly. The problem is that standard test patterns put all database operations in one large transaction, while production creates separate transactions for each operation. This mismatch can hide bugs related to transaction boundaries and constraint timing.

The inherited solution was complex -- patch SQLAlchemy's internal transaction management to automatically use savepoints (nested transactions) instead of the main transaction, making each operation behave like a separate request.

```python
def _test_patch_db_session(global_tx: SessionTransaction) -> Session:
    """
    Patches autobegin, begin, commit, and rollback behavior of the session
    the given top-level (test-function-scope) transaction belongs to
    such that all activity takes place in savepoints (nested transactions), simulating
    per-request session behavior that would take place in production.
    Also, useful as per-test-function session behavior to ensure the db state
    is clean between tests.

    For safety, this implementation prevents the provided global transaction from
    being committed or rolled back by tests.

    For parity: The default SQLAlchemy behavior is to automatically call commit() on top-level
    transactions only. Since we're simulating top-level transactions using savepoints,
    the patch session will attempt to call commit() during close() provided the current
    savepoint is a direct child of the global transaction. That is, commit() would *not*
    automatically be called for a deeply nested savepoint.

    Returns: the provided transaction's Session but with rollback() and commit() modified internally
    """
    session: Session = global_tx.session

    def _patched_begin(self, nested: bool = False) -> SessionTransaction:
        unpatched_method = Session.begin.__get__(self, Session)
        newtx = unpatched_method(True)  # always nested=True for testing
        return newtx

    def _patched__autobegin_t(self, begin: bool = False) -> SessionTransaction:
        assert not begin, "autobegin should not have been asked to start a new TX"
        unpatched_method = Session._autobegin_t.__get__(self, Session)
        newtx = unpatched_method(begin)
        if newtx is global_tx:
            # autobegin doesn't know we want to force nested transactions,
            # so if it returns the existing global tx, force a new savepoint to be created.
            # various operations can cause autobegin, such as just issuing queries without
            # an explicit session.begin(), or even a session.flush() statement
            newtx = _patched_begin(self)
        return newtx

    def _patched_rollback(self) -> None:
        current = self._transaction
        assert current.nested, "should not be rolling back top-level transaction"
        current.rollback()

    def _patched_commit(self) -> None:
        current = self._transaction
        assert current.nested, "should not be committing top-level transaction"
        current.commit()
        # sqlalchemy's sessionmaker by default sets 'expire_on_commit=True'
        # so emulate that here
        session.expire_all()

    def _patched_close(self) -> None:
        current = self._transaction
        if current.nested:
            # SQLAlchemy's behavior when calling close() on a nested transaction
            # is different from close on the top-level. For a nested transaction,
            # close() just discards the changes, but for a top-level transaction
            # close() will attempt to commit() any uncommitted changes, else rollback().
            #
            # For parity, if we're in a nested transaction that is an immediate child
            # of the top-level transaction, and it is still active, attempt to commit
            # (or rollback if it cannot be committed) the savepoint before closing
            try:
                if current._transaction_is_active and current._parent is global_tx:
                    try:
                        current.commit()
                    except Exception:
                        if current._rollback_can_be_called():
                            current.rollback()
                current.close()
            finally:
                # everything should be 'closed'; ensure the state is reset
                session.expire_all()
        # it is possible to call close() on the top-level transaction,
        # especially in cases where the only operation was an auto-begun
        # read or if there were no queries issued at all
        else:
            # everything should be 'closed'; ensure the state is reset
            session.expire_all()
            assert (
                current._state == SessionTransactionState.ACTIVE
            ), f"attempting to close the global transaction in non-ACTIVE state {current._state}"

    session._autobegin_t = _patched__autobegin_t.__get__(session, Session)
    session.begin = _patched_begin.__get__(session, Session)
    session.commit = _patched_commit.__get__(session, Session)
    session.rollback = _patched_rollback.__get__(session, Session)
    session.close = _patched_close.__get__(session, Session)

    return session
```

The purpose of this patch function was to enforce nested transactions (savepoints) for every database operation, isolating test runs from each other such that a rollback in one does not affect another. Rollbacks clean up each test's commits in isolation, preventing the destruction of data in other tests, and allowing clean databases for new tests to use, circumventing the tedious work of creation and deletion of tables across test cases.

The concept being, instead of all test operations sharing one large transaction, each operation gets its own savepoint that can succeed or fail independently, much like separate HTTP requests in production.

For reference, the [SQLAlchemy Savepoint docs](https://docs.sqlalchemy.org/en/20/orm/session_transaction.html#using-savepoint) explain how nested transactions work within a larger transaction context.

## Understanding SQLAlchemy's autobegin behavior

To understand the failure, I needed to dive into how SQLAlchemy manages transactions automatically. Based on the [SQLAlchemy session_transaction](https://docs.sqlalchemy.org/en/20/orm/session_transaction.html#explicit-begin) docs, without explicit `Session.begin()` calls, "autobegin" behavior starts transactions where needed even when not explicitly invoked:

> The Session features "autobegin" behavior, meaning that as soon as operations begin to take place, it ensures a SessionTransaction is present to track ongoing operations.

Our patch modified this behavior through the `_patched__autobegin_t` function to automatically return savepoints for every transaction request, including the ones implicitly started for our crud functions. This means when SQLAlchemy internally calls `_autobegin_t()` to ensure a transaction exists, our patch intercepts that call and provides a savepoint instead of the main transaction.

The key insight is that `_autobegin_t()` serves two purposes:
1. **Normal operations**: Ensure transactions exist when database work needs to happen
2. **Recovery operations**: Provide appropriate transaction access during error handling

## The problem and learnings

The problem arises when your program, mid-transaction, encounters a database error (such as a constraint violation like a duplicate primary key insert). When this happens, SQLAlchemy's session becomes [DEACTIVE](https://github.com/sqlalchemy/sqlalchemy/blob/main/lib/sqlalchemy/orm/session.py#L965-L974) and must attempt to recover.

From the [Rolling Back SQLAlchemy docs](https://docs.sqlalchemy.org/en/20/orm/session_basics.html#rolling-back):

> When a Session.flush() fails, typically for reasons like primary key, foreign key, or "not nullable" constraint violations, a ROLLBACK is issued automatically... However, the Session goes into a state known as "inactive" at this point, and the calling application must always call the Session.rollback() method explicitly so that the Session can go back into a usable state.

Based on [What does the Session do?](https://docs.sqlalchemy.org/en/20/orm/session_basics.html#what-does-the-session-do), the [session transaction is not deactivated when rollback fails](https://github.com/sqlalchemy/sqlalchemy/issues/4050) GitHub issue, and [Using SAVEPOINT](https://docs.sqlalchemy.org/en/20/orm/session_transaction.html#using-savepoint) documentation, I learned that rollbacks on savepoints only emit "ROLLBACK TO SAVEPOINT" which only rollback the savepoints' portion of the transaction state rather than the whole transaction.

The critical insight is that the Session manages the primary database connection transaction and has administrative capabilities that savepoints don't have. When catastrophic errors occur, SQLAlchemy needs these capabilities to restore session state:

- **Complete object registry**: The main transaction tracks all objects loaded during the entire session
- **Full rollback authority**: Can reset everything back to the session start  
- **Connection management**: Direct control over database connections and connection state
- **State restoration**: Can mark the session as healthy (ACTIVE) again after corruption

Savepoints have limited scope -- they only know about objects and changes within their own checkpoint boundaries.

Because our patch only hands out savepoints due to our interception of `_autobegin_t`, in the event of a catastrophic error, SQLAlchemy cannot obtain the main transaction it needs to recover properly.

## The error chain

The error chain is as follows:

1. **Catastrophic error occurs**: `delete_user_policy` test hits a database constraint violation, connection loss, or similar error that corrupts session state
2. **Session becomes DEACTIVE**: SQLAlchemy marks the session as unusable due to the corruption
3. **Recovery attempt**: SQLAlchemy tries to recover by calling `_autobegin_t()` to access the main transaction  
4. **Patch interference**: Our patch intercepts this call and gives it a savepoint instead, which lacks the privileges needed for session-level recovery
5. **Recovery fails**: Unable to restore session state, the session remains DEACTIVE
6. **Cleanup failure**: Test cleanup tries to rollback the main transaction, but it's in an unusable state → "InvalidRequestError: This session is inactive"

The failure manifests during test cleanup, but the root cause is that SQLAlchemy couldn't access the main transaction during the recovery attempt.

## The solution

The solution is to update `_patched__autobegin_t` to be aware of the session state and allow SQLAlchemy to access the main transaction when recovery is needed:

```python
def _patched__autobegin_t(self, begin: bool = False) -> SessionTransaction:
    assert not begin, "autobegin should not have been asked to start a new TX"
    unpatched_method = Session._autobegin_t.__get__(self, Session)
    newtx = unpatched_method(begin)
    if newtx is global_tx:
        # autobegin doesn't know we want to force nested transactions,
        # so if it returns the existing global tx, force a new savepoint to be created.
        # various operations can cause autobegin, such as just issuing queries without
        # an explicit session.begin(), or even a session.flush() statement

        # In addition, in a catastrophic error state where SQLAlchemy requires the
        # main transaction with higher privileges to restore its state safely, we must
        # ensure that in the case where the global_tx state is not active, that SQLAlchemy
        # can fall back to its normal autobegin_t behavior to grab the main transaction
        # and save itself
        if global_tx._state == SessionTransactionState.ACTIVE:
            newtx = _patched_begin(self)

    return newtx
```

This change preserves the testing behavior for normal operations while providing an "escape hatch" for SQLAlchemy's recovery mechanisms. When the main transaction is healthy (`ACTIVE`), the patch continues to provide savepoints for realistic testing. When the session is corrupted (not `ACTIVE`), the patch lets SQLAlchemy access the main transaction for recovery.

