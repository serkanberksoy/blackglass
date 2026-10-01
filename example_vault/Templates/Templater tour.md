# <% tp.file.title %>

Templater's functions, in one template ("Templater: Insert template").

- Today: <% tp.date.now("dddd, MMMM Do YYYY") %>
- This template: <% tp.config.template_file.basename %> (run mode <% tp.config.run_mode %>)
- Colors: <% (await tp.system.multi_suggester(["Red", "Green", "Blue"], ["red", "green", "blue"], false, "Colors (Tab marks)")).join(", ") %>
- Notes: <% await tp.system.prompt("A few lines (Alt+Enter)", "", false, true) %>

<% tp.user.callout("Scripts in Scripts/ are tp.user.<file name>") %>

<% tp.web.daily_quote() %>

<% tp.file.cursor(1) %>
