// A Dataview view: a table of the books in `input.folder`, best first.
const books = dv.pages(`"${input?.folder ?? "Books"}"`).where(p => p.rating);
dv.table(["Book", "Author", "Stars"],
  books.sort(p => p.rating, "desc").map(p => [p.file.link, p.author, "★".repeat(p.rating)]));
