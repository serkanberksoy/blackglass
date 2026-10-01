// A user script: Templater makes it tp.user.callout(text, kind).
module.exports = function (text, kind = "tip") {
  return `> [!${kind}]\n> ${text}`;
};
