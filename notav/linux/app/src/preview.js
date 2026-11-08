// Preview mode's wide tables and code blocks (src/preview.rs), as Editor mode's strips: each
// scrolls sideways on its own, a shade at the edge where more of it is hidden, its scrollbar at
// the bottom of what is on screen while the block goes on below it. With Wrap Lines off the
// whole page scrolls instead (the `nowrap` class).
(() => {
  if (document.documentElement.classList.contains("nowrap")) return;
  for (const block of document.querySelectorAll(".markdown-body pre, .markdown-body table")) {
    const strip = document.createElement("div");
    strip.className = "strip";
    block.replaceWith(strip);
    strip.append(block);
    const bar = document.createElement("div");
    bar.className = "strip-bar";
    const width = document.createElement("div");
    bar.append(width);
    strip.append(bar);
    const edges = () => {
      const wide = !bar.hidden;
      strip.classList.toggle("more-left", wide && block.scrollLeft > 0.5);
      strip.classList.toggle(
        "more-right",
        wide && block.scrollLeft + block.clientWidth < block.scrollWidth - 1,
      );
    };
    const show = () => {
      const wide = block.scrollWidth > block.clientWidth + 1;
      bar.hidden = !wide;
      // The bar's range as the block's, so both end together.
      width.style.width = `${block.scrollWidth - block.clientWidth + bar.clientWidth}px`;
      strip.style.setProperty("--bar", `${wide ? bar.offsetHeight : 0}px`);
      edges();
    };
    // Each follows the other, skipping the scroll it was given: setting a position stops a
    // smooth scroll going on (a wheel's), which would then move a step a frame.
    let toBar = null;
    let toBlock = null;
    block.addEventListener("scroll", () => {
      edges();
      if (toBlock !== null && Math.abs(block.scrollLeft - toBlock) < 1) return;
      toBlock = null;
      toBar = block.scrollLeft;
      bar.scrollLeft = toBar;
    });
    bar.addEventListener("scroll", () => {
      if (toBar !== null && Math.abs(bar.scrollLeft - toBar) < 1) return;
      toBar = null;
      toBlock = bar.scrollLeft;
      block.scrollLeft = toBlock;
    });
    new ResizeObserver(show).observe(block);
    show();
  }
})();
