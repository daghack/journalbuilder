//! Books: pages numbered in order, named for their number and section, and
//! a contents of where each section starts.
//!
//! ```rhai
//! let book = new_book(#{ first: 3 });       // the first page added is page 3
//! let folio = book.next;                    // the number the next page gets
//! book.add("pens", page);                   // named 03_pens; gives its number
//! book.add("layouts", typeset_pages);       // several pages, in order
//! book.contents;                            // [["pens", 3], ["layouts", 4]]
//! journal_entry.add(book.pages);
//! ```

use rhai::{Array, CustomType, Dynamic, EvalAltResult, Map, NativeCallContext, TypeBuilder, INT};

use super::entry::{check_name, Page};
use super::options::apply_props;
use super::RhaiResult;

#[derive(Debug, Clone, CustomType)]
#[rhai_type(extra = Self::build_extra)]
pub struct Book {
    #[rhai_type(skip)]
    first: INT,
    #[rhai_type(skip)]
    pages: Vec<Page>,
    /// Each section and the number of its first page.
    #[rhai_type(skip)]
    contents: Vec<(String, INT)>,
}

impl Book {
    fn new() -> Self {
        Book { first: 1, pages: Vec::new(), contents: Vec::new() }
    }
    fn next(&mut self) -> INT {
        self.first + self.pages.len() as INT
    }
    /// `book.add(section, page)`: the page named `NN_section`, its number
    /// (from `first`) padded to two digits and the section's spaces made
    /// underscores. Gives the page's number.
    fn add(&mut self, section: &str, mut page: Page) -> RhaiResult<INT> {
        let number = self.next();
        page.name = Some(check_name("book.add", Dynamic::from(format!("{:02}_{}", number, section.replace(' ', "_"))))?);
        self.pages.push(page);
        if self.contents.last().is_none_or(|(last, _)| last != section) {
            self.contents.push((section.to_string(), number));
        }
        Ok(number)
    }
    /// `book.add(section, pages)`: each page in turn; gives the first's number.
    fn add_all(&mut self, section: &str, pages: Array) -> RhaiResult<INT> {
        let first = self.next();
        for (i, page) in pages.into_iter().enumerate() {
            let type_name = page.type_name();
            let page = page.try_cast::<Page>().ok_or_else(|| -> Box<EvalAltResult> { format!("book.add: item {} is a {}, not a page", i, type_name).into() })?;
            self.add(section, page)?;
        }
        Ok(first)
    }
    fn get_first(&mut self) -> INT {
        self.first
    }
    fn set_first(&mut self, first: INT) -> RhaiResult<()> {
        if !self.pages.is_empty() {
            return Err("book.first: set it before adding pages".into());
        }
        self.first = first;
        Ok(())
    }
    fn build_extra(builder: &mut TypeBuilder<Self>) {
        builder
            .with_name("Book")
            .with_fn("new_book", Self::new)
            .with_fn("new_book", |ctx: NativeCallContext, options: Map| apply_props(&ctx, "new_book", Book::new(), options, &["first"]))
            .with_fn("add", Self::add)
            .with_fn("add", Self::add_all)
            .with_get_set("first", Self::get_first, Self::set_first)
            .with_get("next", Self::next)
            .with_get("pages", |b: &mut Self| -> Array { b.pages.iter().cloned().map(Dynamic::from).collect() })
            .with_get("contents", |b: &mut Self| -> Array {
                b.contents.iter().map(|(section, n)| Dynamic::from_array(vec![section.clone().into(), Dynamic::from(*n)])).collect()
            });
    }
}

/// `roman(n)`: n in lowercase roman numerals, for folios.
pub(super) fn roman(n: INT) -> RhaiResult<String> {
    if !(1..=3999).contains(&n) {
        return Err(format!("roman: n should be from 1 to 3999, got {}", n).into());
    }
    let mut n = n;
    let mut out = String::new();
    for (value, numeral) in [(1000, "m"), (900, "cm"), (500, "d"), (400, "cd"), (100, "c"), (90, "xc"), (50, "l"), (40, "xl"), (10, "x"), (9, "ix"), (5, "v"), (4, "iv"), (1, "i")] {
        while n >= value {
            out.push_str(numeral);
            n -= value;
        }
    }
    Ok(out)
}

/// `escape_markdown(text)`: text that `markdown()` sets as it is, every
/// mark escaped.
pub(super) fn escape_markdown(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if "\\*_~[]{}".contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}
