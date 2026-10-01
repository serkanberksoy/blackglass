---
author: <% tp.system.prompt("Author") %>
rating: <% tp.system.suggester(["★", "★★", "★★★", "★★★★", "★★★★★"], [1, 2, 3, 4, 5]) %>
tags: [reading]
---
# <% tp.file.title %>

Started <% tp.date.now("dddd, MMMM Do") %>.
<% tp.file.cursor() %>
