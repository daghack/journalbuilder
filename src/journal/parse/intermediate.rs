use crate::symbol::Symbol;

#[derive(Debug, Clone)]
pub struct IntermediateContent<'md> {
    pub markdown: &'md str,
    pub content: Vec<ParsedContent<'md, JournalContent<'md>>>,
}

impl <'md> IntermediateContent<'md> {
    pub fn journal_content(&self) -> Vec<&JournalContent<'md>> {
        self.content.iter().map(|c| &c.content).collect()
    }
    pub fn into_journal_content(self) -> Vec<JournalContent<'md>> {
        self.content.into_iter().map(|c| c.content).collect()
    }
}

#[derive(Debug, Clone)]
pub enum JournalContent<'src> {
    TextBlock {
        styled_text: Vec<ParsedContent<'src, TextStylizing>>,
    },
    CodeBlock {
        language: Option<&'src str>,
        content: &'src str,
    },
    CanvasEmbed {
        alt: &'src str,
        url: &'src str,
    },
    UnplacedSymbols {
        symbols: Vec<(Symbol, TextStylizing)>,
    },
    ThematicBreak,
}

impl <'src> JournalContent<'src> {
    pub fn get_style_of_byte_offset(&self, offset: usize) -> Option<&TextStylizing> {
        match self {
            JournalContent::TextBlock { styled_text } => {
                for styled in styled_text.iter() {
                    if offset >= styled.offset && offset < styled.offset + styled.content_slice.len() {
                        return Some(&styled.content);
                    }
                }
                None
            },
            _ => None,
        }
    }
    pub fn get_style_of_char_offset(&self, index: usize) -> Option<&TextStylizing> {
        match self {
            JournalContent::TextBlock { styled_text } => {
                let mut counted_chars = 0;
                for styled in styled_text.iter() {
                    let styled_length = styled.content_slice.chars().count();
                    if index < counted_chars + styled_length {
                        return Some(&styled.content);
                    }
                    counted_chars += styled_length;
                }
                None
            },
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ParsedContent<'src, Content> {
    pub content_slice: &'src str,
    pub offset: usize,
    pub content: Content,
}

#[derive(Debug, Clone, Default)]
pub struct TextStylizing {
    pub strikethrough: bool,
    pub underline: bool,
    pub italic: bool,
    pub bold: bool,
    pub code: bool,
    pub color: Option<String>,
    pub heading_level: Option<usize>,
    pub scaling: Option<f32>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TextTokenType {
    Word,
    InlineWhitespace{width_scale: f32},
    Newline,
}

#[derive(Debug, Clone)]
pub struct TextToken <'join> {
    pub token_type: TextTokenType,
    pub content: Vec<ParsedContent<'join, TextStylizing>>,
}

pub fn styled_text_to_tokens<'src>(styled_text: &Vec<ParsedContent<'src, TextStylizing>>) -> Vec<TextToken<'src>> {
    let mut tokens = Vec::new();
    // keep track of the current token between styled boundaries.
    let mut overflow_word_token = Vec::new();
    for styled in styled_text.iter() {
        //content slice
        //global offset
        //content (style)
        let mut local_word_index = None;
        for (i, c) in styled.content_slice.char_indices() {
            let token_type = match c {
                '\n' => TextTokenType::Newline,
                ' ' => TextTokenType::InlineWhitespace{width_scale: 1.0},
                '\t' => TextTokenType::InlineWhitespace{width_scale: 4.0},
                _ => TextTokenType::Word,
            };
            if token_type == TextTokenType::Word {
                local_word_index.get_or_insert(i);
            } else {
                let overflow_content : Vec<_> = overflow_word_token.drain(..).collect();
                if !overflow_content.is_empty() {
                    tokens.push(TextToken {
                        token_type: TextTokenType::Word,
                        content: overflow_content,
                    });
                }
                if let Some(word_start) = local_word_index {
                    let global_index = styled.offset + word_start;
                    let parsed_content = ParsedContent {
                        content_slice: &styled.content_slice[word_start..i],
                        offset: global_index,
                        content: styled.content.clone(),
                    };
                    tokens.push(TextToken {
                        token_type: TextTokenType::Word,
                        content: vec![parsed_content],
                    });
                    local_word_index = None;
                }
                let global_index = styled.offset + i;
                let parsed_content = ParsedContent {
                    content_slice: &styled.content_slice[i..i+c.len_utf8()],
                    offset: global_index,
                    content: styled.content.clone(),
                };
                tokens.push(TextToken {
                    token_type,
                    content: vec![parsed_content],
                });
            }
        }
        if let Some(word_start) = local_word_index {
            let global_index = styled.offset + word_start;
            let parsed_content = ParsedContent {
                content_slice: &styled.content_slice[word_start..],
                offset: global_index,
                content: styled.content.clone(),
            };
            overflow_word_token.push(parsed_content);
        }
    }
    // if we ended with an open token, we need to close it
    // this can happen if the last part of the styled text is a word and not whitespace or newline.
    if !overflow_word_token.is_empty() {
        tokens.push(TextToken {
            token_type: TextTokenType::Word,
            content: overflow_word_token,
        });
    }
    tokens
}
