<%* const monday = tp.date.weekday("YYYY-MM-DD", 0); -%>
# Week of <% tp.date.weekday("MMMM Do", 0) %>

## Days
<%* for (let d = 0; d < 7; d++) {
  const day = moment(monday).add(d, "days");
  tR += `- [[${day.format("YYYY-MM-DD")}|${day.format("dddd")}]]\n`;
} -%>

## Wins
- <% tp.file.cursor() %>

## Next week
-
