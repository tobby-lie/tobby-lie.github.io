+++
title = "Good intentions, confusing autobegin behavior"
date = 2025-02-21
+++

# Inheriting code-bases
Software engineers come and go, but code remains. When coworkers leave, you inherit their code and may even need to add to it. You learn a lot about how they think and inevitably adopt their mindset in an effort to deconstruct their logic.

# A puzzling error
After inheriting an previous software engineer's service and needing to add a feature, my coworker had identified an interesting error in the unit test suite:

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

# Realistic testing
While examining the `_test_patch_db_session` function, I encountered something I hadn't seen before:

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

Summarized, the (well-intentioned) purpose of this patch function was to enforce nested transactions for every transaction, isolating test runs from eachother such that a rollback in one does not effect another. Rollbacks clean up each test's commits in isolation, preventing the destruction of data in other tests, and allowing clean databases for new tests to use, circumventing the tedious work of creation and deletion of tables across test cases.

In addition, in production, the functions being tested are invoked within endpoints where explicit nested transactions are created, so this patch also simulates real-world behavior when a user makes a requests to this service.

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

`crud.delete_example` here would just use `db.execute` without calling an explicity begin and in the tests we test `crud.delete_example` directly.

[SQLAlchemy Savepoint docs](https://docs.sqlalchemy.org/en/20/orm/session_transaction.html#using-savepoint)

# The problem and learnings
The problem arises when your program, mid transaction, encounters a database error (such as a constraint violation like a duplicate primary key insert). When this happens, SQLAlchemy's session becomes [DEACTIVE](https://github.com/sqlalchemy/sqlalchemy/blob/main/lib/sqlalchemy/orm/session.py#L965-L974) and must attempt to recover.

Based on the SQLAlchemy [session_transaction](https://docs.sqlalchemy.org/en/20/orm/session_transaction.html#explicit-begin) docs, without explit `Session.begin()` calls, "autobegin" behavior, starts transactions where needed even when not explicitly invoked. Recall, in our patch, we modified this behavior to automatically return savepoints for every transaction made (such as the ones implicitly started for our crud functions).

In [session_basics.flushing](https://docs.sqlalchemy.org/en/20/orm/session_basics.html#flushing), it's explained that when a `flush` fails, an explicit call to `Session.rollback()` is made to attempt to clear it's inactive state.
