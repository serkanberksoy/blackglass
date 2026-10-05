// A Dataview view: `await dv.view("Data/views/stars", {rating: 4})`.
const n = Math.round(input?.rating ?? 0);
dv.span("★".repeat(n) + "☆".repeat(5 - n));
