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
    const show = () => {
      const wide = block.scrollWidth > block.clientWidth + 1;
      bar.hidden = !wide;
      width.style.width = `${block.scrollWidth}px`;
      strip.style.setProperty("--bar", `${wide ? bar.offsetHeight : 0}px`);
      strip.classList.toggle("more-left", wide && block.scrollLeft > 0.5);
      strip.classList.toggle(
        "more-right",
        wide && block.scrollLeft + block.clientWidth < block.scrollWidth - 1,
      );
    };
    block.addEventListener("scroll", () => {
      if (bar.scrollLeft !== block.scrollLeft) bar.scrollLeft = block.scrollLeft;
      show();
    });
    bar.addEventListener("scroll", () => {
      if (block.scrollLeft !== bar.scrollLeft) block.scrollLeft = bar.scrollLeft;
    });
    new ResizeObserver(show).observe(block);
    show();
  }
})();
