/// <reference types="tree-sitter-cli/dsl" />

module.exports = grammar({
  name: "astrum_md",

  extras: (_) => [],

  rules: {
    document: ($) => repeat($._block),

    _block: ($) =>
      choice(
        $.heading1,
        $.heading2,
        $.heading3,
        $.heading4,
        $.heading5,
        $.heading6,
        $.subheading,
        $.paragraph,
        $.blank_line,
        $._newline,
      ),

    // Two or more consecutive newlines — a visible paragraph break.
    blank_line: (_) => /\n[ \t]*\n/,

    _newline: (_) => /\n/,

    heading6: ($) => seq("######", /[ \t]+/, $._inline_content, optional("\n")),
    heading5: ($) => seq("#####", /[ \t]+/, $._inline_content, optional("\n")),
    heading4: ($) => seq("####", /[ \t]+/, $._inline_content, optional("\n")),
    heading3: ($) => seq("###", /[ \t]+/, $._inline_content, optional("\n")),
    heading2: ($) => seq("##", /[ \t]+/, $._inline_content, optional("\n")),
    heading1: ($) => seq("#", /[ \t]+/, $._inline_content, optional("\n")),
    subheading: ($) => seq("-#", /[ \t]+/, $._inline_content, optional("\n")),

    paragraph: ($) => seq($._inline_content, optional("\n")),

    _inline_content: ($) => prec.right(repeat1($._inline)),

    _inline: ($) =>
      choice(
        $.code_span,
        $.bold,
        $.italic,
        $.underline,
        $.strikethrough,
        $.link,
        $.text,
      ),

    code_span: ($) =>
      seq("`", alias(token.immediate(/[^`\n]+/), $.code_content), "`"),

    // Bold: always has opening **, content that doesn't start with **, optional closing **.
    // Content is specifically "text that doesn't contain ** at the boundary".
    bold: ($) => prec.right(seq(
      "**",
      alias($._bold_content, $.content),
      optional("**"),
    )),
    _bold_content: (_) => token.immediate(prec(1, /([^*\n]|\*[^*])+/)),

    italic: ($) => prec.right(seq(
      "*",
      alias($._italic_content, $.content),
      optional("*"),
    )),
    _italic_content: (_) => token.immediate(prec(1, /[^*\n]+/)),

    underline: ($) => prec.right(seq(
      "__",
      alias($._underline_content, $.content),
      optional("__"),
    )),
    _underline_content: (_) => token.immediate(prec(1, /([^_\n]|_[^_])+/)),

    strikethrough: ($) => prec.right(seq(
      "~~",
      alias($._strikethrough_content, $.content),
      optional("~~"),
    )),
    _strikethrough_content: (_) => token.immediate(prec(1, /([^~\n]|~[^~])+/)),

    link: (_) => token(prec(1, /\[[^\]\n]+\]\([^)\n]*\)?/)),

    text: (_) => token(prec(-1, /[^\n*_~`#\[]+|[*_~#\[]/)),
  },
});
