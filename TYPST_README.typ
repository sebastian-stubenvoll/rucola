= Typst Notes!

This fork scratches my own personal itch. Somewhat frustrated with writing math in markdown and bothered by
how bloated latex files often end up getting (disproportionally so for small notes!) I recently switched to typst.

I've quickly grown fond of the simplicity of the markup and math code. However some features more commonly found
in the markdown-note-taking-universe were a little harder to some by. I felt I was especially missing a good solution
for note management and convenitenly browsing the existing notes catalogue. While toying with the idea of writing
a TUI myself I stumbled across rucola and it ticked all the boxes. So this is an attempt to integrate typst notes.

= Caveats

Generally for most use cases typst is a lot closer to something like Latex than it is to plain markdown notes.
This leads to some caveats when trying to make typst play nicely with rucola and the zettelkasten philosophy,
which in turn lead to some assumptions that are made as part of the design process. I'll (attempt to) justify those
here, though I am open to suggetions.

== YAML frontmatter
This is pretty much 1:1 taken from markdown, except it is wrapped in a block comment, e.g.
```typst
/*
---
title: Cool Note!
tags:
  - writing
  - productivity
*/
```
Maybe there's a better solution here? This assumes you don't want the front matter to be shown, which is okay if
you're only using it for metadata. One could consider a function based approach here, as done with links and tags.

== Links
Markdown being a lot simpler doesn't tend to explicitly keep compiled files around.
Instead nice looking html/pdf versions are often only compiled on demand or ad hoc.
Typst compile times are pretty fast, however keeping a pdf file around is nice.
But since the two are so clearly seprated links are usually only pointing to other PDFs.
Things get messy when trying to figure out structes and links between source files and
often this is a little tedious to use with path auto-completion. Thus this decision was made:

*Source file notes only reference other source file notes!*

In the config file one defines a linking function.
The linking function can be pretty much anything, the only requirement is that the first argument
be a string that the path to the source file (e.g. ./my-cool-note.typ or ./old-notes/recipes.md)

Rucola will then traverse the typst syntax tree (using the typst parser) looking for that function
and extract those note links as meta data.

N.B. rucola does not look at the evaluated function, merely at the call. So the linking function can still evaluate to pdf links!
Additionally this easily allows for custom styling
For example:
```typst
#let link_note = (path, desc) => {
  let pdf_path = if path.ends-with(".typ") {
    path.replace(regex("typ$"), "pdf")
  } else {
    panic("link_note expects a .typ file!")
  }
  set text(olive)
  link(pdf_path)[#desc]
}

link_note("./my-cool-note.typ")
```

N.B. Only a syntax tree is parsed. None of it is evaluated. This allows the behaviour described above, however it also means
that the defined reference function should be called directly. Indirection may lead to incorrect links!

== Tags
A smiliar approach was chosen for tags. Instead of marking tags with a hashtag (which would be a nightmare
since hashtags are an integral part of the typst syntax) a tag function is defined in the settings.
This way the tags can be easily identified using the typst syntax parser.
The first argument of that function is read as tag. Rucola prepends a hash for internal display if there isn't one present
at the beginning of the string.

This comes with the same perks as the link-approach, i.e. tags can be styled or even hidden from the rendered pdf, e.g.
```typst
let tag = text => {
  hidden(text)
}

// or in shortform
let tag = text => {}


// Rucola will parse this as #metadata-only but it won't render in the final pdf!
#tag("metadata-only")
```


P.S. it felt fitting to make this a typst rather than a markdown file.
