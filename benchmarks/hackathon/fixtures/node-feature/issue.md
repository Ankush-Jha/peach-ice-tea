# Add title-case formatting to `lib.js`

The display layer needs a `titleCase` helper in `lib.js` that formats free-text labels: it
should capitalize the first letter of every word and lowercase the rest of each word, e.g.
`"hello WORLD"` becomes `"Hello World"`.

`titleCase` currently just returns its input unchanged. Implement it so it satisfies the
existing test suite in `lib.test.js`.
