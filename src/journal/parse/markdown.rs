use markdown::mdast::{Node, Paragraph};
use markdown::unist::Position;
use super::intermediate::{IntermediateContent, JournalContent, ParsedContent, TextStylizing};

fn indexed_content<'a>(source: &'a str, position: &Position) -> &'a str {
    &source[position.start.offset..position.end.offset]
}

fn indexed_content_from_char_offset<'a>(source: &'a str, offset: usize, length: usize) -> &'a str {
    let character_iter = source.char_indices()
        .skip(offset)
        .take(length)
        .collect::<Vec<(usize, char)>>();
    let start = character_iter.first().map(|(idx, _)| *idx).unwrap_or(offset);
    let end = character_iter.last().map(|(idx, c)| *idx + c.len_utf8()).unwrap_or(offset + length);
    &source[start..end]
}
fn indexed_content_from_byte_offset<'a>(source: &'a str, offset: usize, length: usize) -> &'a str {
    &source[offset..offset + length]
}

fn parse_link_formatting(link: &str) -> TextStylizing {
    let mut formatting = TextStylizing::default();
    for part in link.split(';') {
        let part = part.trim();
        if part.starts_with("color:") {
            formatting.color = Some(part[6..].replace("<", "(").replace(">", ")").to_string());
        } else if part.starts_with("scale:") {
            if let Ok(scale) = part[6..].parse::<f32>() {
                formatting.scaling = Some(scale);
            }
        } else {
            match part {
                "bold" => formatting.bold = true,
                "italic" => formatting.italic = true,
                "underline" => formatting.underline = true,
                "strikethrough" => formatting.strikethrough = true,
                _ => println!("Unknown link formatting option: '{}'", part),
            }
        }
    }
    formatting
}

fn determine_node_formatting<'md>(source: &'md str, node: &Node) -> (TextStylizing, usize, usize) {
    let mut formatting = TextStylizing::default();
    let position = node.position().unwrap();
    let offset = position.start.offset;
    let start_char = source.chars().nth(offset).unwrap_or('\0');
    let mut remove_first = false;
    match node {
        Node::Emphasis(_emphasis) => {
            match start_char {
                '*' => {
                    formatting.italic = true;
                    remove_first = true;
                },
                '_' => {
                    formatting.underline = true;
                    remove_first = true;
                }
                _ => println!("\tEmphasis with unknown marker '{}'", start_char),
            }
        }
        Node::Strong(_strong) => {
            formatting.bold = true;
            remove_first = true;
        }
        Node::Delete(_delete) => {
            formatting.strikethrough = true;
            remove_first = true;
        }
        Node::InlineCode(_code) => {
            formatting.code = true;
            remove_first = true;
        }
        Node::Link(link) => {
            formatting = parse_link_formatting(&link.url);
        }
        Node::Heading(_heading) => {
            let content = indexed_content(source, position);
            let leading_hashes = content.chars().take_while(|c| *c == '#').count();
            formatting.heading_level = Some(leading_hashes);
        }
        _ => {}
    }
    if remove_first {
        // count the number of [start_char]s at the start of the content and adjust the position
        // accordingly, adjusting 
        let content = indexed_content(source, position);
        let mut char_count = 0;
        for c in content.chars() {
            if c == start_char {
                char_count += 1;
            } else {
                break;
            }
        }
        (formatting, offset + char_count, content.len() - char_count*2)
    } else {
        (formatting, offset, position.end.offset - position.start.offset)
    }
}

fn parse_paragraph_elements<'md>(source: &'md str, paragraph: &Paragraph) -> Vec<ParsedContent<'md, JournalContent<'md>>> {
    let mut canvas_content = Vec::new();
    let mut current_text_nodes: Vec<ParsedContent<'md, TextStylizing>> = Vec::new();
    for child in paragraph.children.iter() {
        let (format, offset, len) = determine_node_formatting(source, child);
        if let Some(position) = child.position() {
            match child {
                Node::Image(_image) => {
                    if !current_text_nodes.is_empty() {
                        let start_index = current_text_nodes.first().unwrap().offset;
                        let last_node = current_text_nodes.last().unwrap();
                        let end_index = last_node.offset + last_node.content_slice.len();
                        let tb = JournalContent::TextBlock {
                            styled_text: current_text_nodes,
                        };
                        canvas_content.push(ParsedContent {
                            content_slice: &source[start_index..end_index],
                            offset: start_index,
                            content: tb,
                        });
                        current_text_nodes = Vec::new();
                    }
                    let content_slice = indexed_content(source, position);
                    // Extract alt and url from the content slice, which is in the format ![alt](url)
                    let alt_start = content_slice.find('[').map(|idx| idx + 1).unwrap_or(0);
                    let alt_end = content_slice.find(']').unwrap_or(content_slice.len());
                    let url_start = content_slice.find('(').map(|idx| idx + 1).unwrap_or(0);
                    let url_end = content_slice.find(')').unwrap_or(content_slice.len());
                    let alt = &content_slice[alt_start..alt_end];
                    let url = &content_slice[url_start..url_end];
                    let cb = JournalContent::CanvasEmbed {
                        alt,
                        url,
                    };
                    canvas_content.push(ParsedContent {
                        content_slice,
                        offset: position.start.offset,
                        content: cb,
                    });
                }
                Node::Link(link) => {
                    let mut start = position.end.offset;
                    let mut end = position.start.offset;
                    for child in link.children.iter() {
                        if let Some(child_pos) = child.position() {
                            if child_pos.start.offset < start {
                                start = child_pos.start.offset;
                            }
                            if child_pos.end.offset > end {
                                end = child_pos.end.offset;
                            }
                        }
                    }
                    let indexed_content_str = indexed_content_from_byte_offset(source, start, end - start);
                    current_text_nodes.push(ParsedContent {
                        content_slice: indexed_content_str,
                        offset: position.start.offset + 1,
                        content: format,
                    });
                }
                _ => {
                    let indexed_content_str = indexed_content_from_byte_offset(source, offset, len);
                    current_text_nodes.push(ParsedContent {
                        content_slice: indexed_content_str,
                        offset: position.start.offset,
                        content: format,
                    });
                },
            };
        }
    }
    if !current_text_nodes.is_empty() {
        let start_index = current_text_nodes.first().unwrap().offset;
        let last_node = current_text_nodes.last().unwrap();
        let end_index = last_node.offset + last_node.content_slice.len();
        let tb = JournalContent::TextBlock {
            styled_text: current_text_nodes,
        };
        canvas_content.push(ParsedContent {
            content_slice: &source[start_index..end_index],
            offset: start_index,
            content: tb,
        });
    }
    canvas_content
}

fn parse_codeblock<'md>(source: &'md str, code: &markdown::mdast::Code) -> JournalContent<'md> {
    let position = code.position.as_ref().expect("Code block has no position");
    let full_content = indexed_content(source, code.position.as_ref().unwrap());
    // parse the language from the content, which is the first line of the code block
    let mut lines = full_content.lines();
    let first_line = lines.next().unwrap_or("").trim_start_matches('`').trim_end_matches('`');
    let language = if first_line.is_empty() {
        None
    } else {
        Some(first_line)
    };
    let inner_content_start = position.start.offset + full_content.find('\n').map(|idx| idx + 1).unwrap_or(0);
    let last_backticks_start = full_content.rfind("```").unwrap_or(full_content.len());
    let mut inner_content_end = position.start.offset + last_backticks_start;
    // if the inner content ends with a newline, we should exclude that from the content
    // however, if the content is blank, then there is only a single newline which has already been
    // excluded by the inner_content_start calculation, so we should not exclude another newline in
    // that case
    if inner_content_start != inner_content_end {
        inner_content_end -= 1;
    }
    JournalContent::CodeBlock {
        language,
        content: &source[inner_content_start..inner_content_end],
    }
}

fn parse_node_content<'md>(source: &'md str, node: &Node) -> Option<Vec<ParsedContent<'md, JournalContent<'md>>>> {
    match node {
        Node::Heading(heading) => {
            let format = determine_node_formatting(&source, node).0;
            let position = heading.position.as_ref().unwrap();
            // get the content slice for the heading, excluding the leading '#' characters and any
            // leading whitespace after the '#' characters, and excluding any trailing whitespace
            // also adjust the offset accordingly
            let heading_indexed_content = indexed_content(source, position);
            // we also have to calculate the offset of the stripped content, which is the original
            // offset plus the number of '#' characters and any leading whitespace
            let leading_hashes = heading_indexed_content.chars().take_while(|c| *c == '#').count();
            let leading_whitespace = heading_indexed_content.chars().skip(leading_hashes).take_while(|c| c.is_whitespace()).count();
            let total_leading = leading_hashes + leading_whitespace;

            let content = indexed_content(
                source,
                &Position {
                    start: markdown::unist::Point {
                        line: position.start.line,
                        column: position.start.column + total_leading,
                        offset: position.start.offset + total_leading,
                    },
                    end: position.end.clone(),
                }
            );
            let tb = JournalContent::TextBlock {
                styled_text: vec![ParsedContent {
                    content_slice: content,
                    offset: position.start.offset + total_leading,
                    content: format,
                }],
            };
            Some(vec![ParsedContent {
                content_slice: content,
                offset: position.start.offset + total_leading,
                content: tb,
            }])
        },
        Node::Paragraph(paragraph) => {
            let elements = parse_paragraph_elements(source, paragraph);
            Some(elements)
        },
        Node::ThematicBreak(_) => {
            let position = node.position().unwrap();
            let content_slice = indexed_content(source, &position);
            Some(vec![ParsedContent {
                content_slice,
                offset: position.start.offset,
                content: JournalContent::ThematicBreak,
            }])
        },
        Node::Code(code) => {
                let position = node.position().unwrap();
                let content_slice = indexed_content(source, &position);
                Some(vec![ParsedContent {
                    content_slice,
                    offset: position.start.offset,
                    content: parse_codeblock(source, code),
                }])
        }
        _ => None,
    }
}

pub fn parse_markdown_content<'md>(content: &'md str) -> IntermediateContent<'md> {
    let mdast_root = markdown::to_mdast(
        content, &markdown::ParseOptions::gfm())
        .expect("Failed to parse markdown");
    let children = mdast_root.children()
        .expect("Markdown Root has no children");
    let mut parsed_content = Vec::new();
    for child in children {
        if let Some(canvas_content) = parse_node_content(content, child) {
            parsed_content.extend(canvas_content);
        }
    }
    IntermediateContent {
        markdown: content,
        content: parsed_content,
    }
}
