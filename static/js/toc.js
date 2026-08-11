document.addEventListener('DOMContentLoaded', () => {
  const tocLinks = document.querySelectorAll('.toc-nav a');
  if (!tocLinks.length) return;

  const tracked = [];
  tocLinks.forEach((link) => {
    const id = link.getAttribute('href').split('#')[1];
    const heading = id ? document.getElementById(id) : null;
    if (heading) tracked.push({ link, heading });
  });
  if (!tracked.length) return;

  const setActive = (link) => {
    tocLinks.forEach((l) => l.classList.remove('active'));
    if (link) link.classList.add('active');
  };

  tocLinks.forEach((link) => {
    link.addEventListener('click', () => setActive(link));
  });

  const observer = new IntersectionObserver(
    (entries) => {
      const visible = entries.filter((entry) => entry.isIntersecting);
      if (visible.length === 0) return;

      const topMost = visible.reduce((a, b) =>
        a.boundingClientRect.top < b.boundingClientRect.top ? a : b
      );
      const match = tracked.find((t) => t.heading === topMost.target);
      if (match) setActive(match.link);
    },
    { rootMargin: '0px 0px -70% 0px', threshold: 0 }
  );

  tracked.forEach(({ heading }) => observer.observe(heading));
});
