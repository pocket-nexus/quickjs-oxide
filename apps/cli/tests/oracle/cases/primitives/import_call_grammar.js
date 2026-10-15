// mode: print
// case: one-argument dynamic import
if (false) { import('fixture'); }
// case: dynamic import across a line break
if (false) { import
('fixture'); }
// case: dynamic import with a trailing comma
if (false) { import('fixture',); }
// case: two-argument dynamic import
if (false) { import('fixture', {}); }
// case: two-argument dynamic import with a trailing comma
if (false) { import('fixture', {},); }
// case: dynamic import followed by a member suffix
if (false) { import('fixture').then; }
// case: dynamic import followed by an optional member suffix
if (false) { import('fixture')?.then; }
// case: dynamic import followed by an optional computed suffix
if (false) { import('fixture')?.[0]; }
// case: dynamic import followed by an optional call suffix
if (false) { import('fixture')?.(); }
// case: dynamic import followed by a tagged template
if (false) { import('fixture')`tag`; }
// case: parenthesized dynamic import as a new target
if (false) { new (import('fixture')); }
// case: dynamic import as a constructor argument
if (false) { new C(import('fixture')); }
// case: sloppy dynamic import options yield identifier
if (false) { import('module', yield); }
// case: generator dynamic import options yield expression
if (false) { function* load(){ import('module', yield); }; }
