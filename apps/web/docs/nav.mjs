// The docs information architecture. scripts/web/build-site.mjs renders one
// page per slug from apps/web/docs/<slug>.md and builds the sidebar from this.
export const DOC_NAV = [
  {
    title: "Start",
    items: [
      { slug: "embed", title: "Embed quickjs-oxide" },
      { slug: "cli", title: "Use the qjs command line" },
    ],
  },
  {
    title: "Guides",
    items: [
      { slug: "values", title: "Values and calls" },
      { slug: "host", title: "Host services" },
      { slug: "modules", title: "Modules" },
      { slug: "jobs", title: "Promises and jobs" },
      { slug: "limits", title: "Errors and limits" },
      { slug: "webassembly", title: "WebAssembly" },
    ],
  },
  {
    title: "Reference",
    items: [
      { slug: "compatibility", title: "JavaScript support" },
      { slug: "performance", title: "Performance" },
      { slug: "optimizations", title: "Optimizations" },
      { slug: "architecture", title: "Architecture" },
      { slug: "resources", title: "Resources" },
    ],
  },
];
