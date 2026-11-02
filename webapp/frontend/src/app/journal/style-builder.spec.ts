import {
  DEFAULT_STYLE, SETTING_GROUPS, SPAN_FIELDS, copyName, isTypeSetting, numberOf, pairOf, readRule, readStyle, ruleValue, spanNameProblem, startingValue, styleFile, stylePreviewScript, withSetting,
} from './style-builder';

describe('style files', () => {
  it('keeps what the form sets, as JSON', () => {
    const file = JSON.parse(styleFile(DEFAULT_STYLE));
    expect(file.color).toBe('#2b1d12');
    expect(file.brush).toBe('Ink Pen');
    expect(file.min_pressure).toBe(0.4);
    expect(file.glyph_position_jitter).toBe(0.02);
    expect(file.size).toBe(26);
  });

  it('leaves out what the engine takes anyway', () => {
    const file = JSON.parse(styleFile(DEFAULT_STYLE));
    for (const key of ['pressure_profile', 'pool_depth', 'passes', 'italic', 'skew', 'underline', 'indent', 'align', 'split_words', 'spans', 'example']) expect(file[key], key).toBeUndefined();
  });

  it('reads back what it wrote', () => {
    const made = { ...DEFAULT_STYLE, passes: 3, italic: true, align: 'center', example: 'Lorem', color: { colors: ['#9b2318', [0.6, '#9b2318'], '#2b1d12'], vertical: true } } as typeof DEFAULT_STYLE;
    expect(readStyle(styleFile(made))).toEqual(made);
    expect(styleFile(readStyle(styleFile(made)))).toBe(styleFile(made));
  });

  it('keeps a gradient as data', () => {
    const color = { colors: ['#9b2318', [0.6, '#9b2318'], '#2b1d12'], vertical: true };
    expect(JSON.parse(styleFile({ ...DEFAULT_STYLE, color } as typeof DEFAULT_STYLE)).color).toEqual(color);
  });

  it('takes the engine\'s default for what a file leaves out', () => {
    const s = readStyle('{ "glyphset": "copper", "size": 72 }');
    expect(s.glyphset).toBe('copper');
    expect([s.color, s.brush, s.brush_size, s.min_pressure, s.pool_depth, s.paragraph_gap, s.clearance]).toEqual(['black', 'Ink Pen', 1.2, 0.3, 0.3, 0.5, 3]);
    expect([s.line_height, s.skew, s.joined, s.layout, s.angle, s.space_width]).toEqual([null, null, null, null, null, null]);
    expect([s.underline, s.split_words, s.spans]).toEqual([false, false, {}]);
  });

  it('keeps every setting a style has', () => {
    const made = {
      ...DEFAULT_STYLE, pool: 0.4, pool_depth: 0.25, pressure_wander_length: 8, glyph_position_jitter: [0.1, 0.2], glyph_scale_factor: 1.5, glyph_outline_gap: 0.1, space_width: 0.4,
      skew: 0.2, joined: false, underline: { count: 2, spacing: 0.3, style: { color: 'red' } }, strike: 1, paragraph_gap: 0.8, indent: 1.5, valign: 'center', fit: true, layout: 'words', angle: 0.1,
      split_words: 3, split_padding: 0.3, break_words: 2, clearance: 5, overhang: 3, spans: { strong: { style: { passes: 2 }, type: { underline: true } } },
    } as typeof DEFAULT_STYLE;
    expect(readStyle(styleFile(made))).toEqual(made);
  });

  it('writes a pair the same both ways as one number', () => {
    expect(JSON.parse(styleFile({ ...DEFAULT_STYLE, glyph_position_jitter: [0.1, 0.1] })).glyph_position_jitter).toBe(0.1);
  });

  it('leaves split padding out unless words are split', () => {
    expect(JSON.parse(styleFile({ ...DEFAULT_STYLE, split_padding: 0.3 })).split_padding).toBeUndefined();
    expect(JSON.parse(styleFile({ ...DEFAULT_STYLE, split_padding: 0.3, split_words: 2 })).split_padding).toBe(0.3);
  });

  it('has a field for every setting a file holds', () => {
    const keys = SETTING_GROUPS.flatMap((group) => group.fields.map((field) => field.key as string));
    const others = ['description', 'spans', 'example'];
    expect([...keys, ...others].sort()).toEqual(Object.keys(DEFAULT_STYLE).sort());
  });

  it('rounds as a slider steps, and passes to a whole number', () => {
    const file = JSON.parse(styleFile({ ...DEFAULT_STYLE, pool: 0.1 + 0.2, passes: 2.4 }));
    expect([file.pool, file.passes]).toEqual([0.3, 2]);
  });

  it('says what\'s wrong with a file', () => {
    for (const [text, wanted] of [
      ['fn style(o) { #{} }', /isn't JSON/],
      ['[]', /JSON object/],
      ['{ "width": 3 }', /'width'/],
      ['{ "size": "big" }', /'size' should be a number/],
      ['{ "color": 3 }', /'color' should be a colour or a gradient/],
      ['{ "fit": 3 }', /'fit' should be a boolean/],
      ['{ "layout": 3 }', /'layout' should be a string/],
      ['{ "split_words": "x" }', /'split_words' should be a number/],
      ['{ "glyph_position_jitter": [1] }', /'glyph_position_jitter' should be a number or \[across, down\]/],
      ['{ "underline": "x" }', /'underline' should be true, false, a number of lines/],
      ['{ "spans": 3 }', /'spans' should be an object/],
    ] as const) expect(() => readStyle(text), text).toThrowError(wanted);
  });

  it('names a copy with a name not taken', () => {
    expect(copyName('ink', () => false)).toBe('ink_copy');
    expect(copyName('ink', (name) => ['ink_copy', 'ink_copy_2'].includes(name))).toBe('ink_copy_3');
  });

  it('previews by name, in its example or lorem ipsum', () => {
    expect(stylePreviewScript('ink', DEFAULT_STYLE, 1000)).toContain('text_style("ink")');
    expect(stylePreviewScript('ink', DEFAULT_STYLE, 1000)).toContain('write_block(content("lorem"), vec2(size, size * 2), 1000.0 - size * 2, settings)');
    expect(stylePreviewScript('ink', { ...DEFAULT_STYLE, example: 'Dolor' }, 1000)).toContain('write_block("Dolor",');
  });

  it('previews typeset, as markdown, its spans among the marks', () => {
    const script = stylePreviewScript('ink', { ...DEFAULT_STYLE, spans: { warn: {}, em: {} } }, 1000, 'typeset');
    expect(script).toContain('roles: #{ text: text_style("ink") }');
    expect(script).toContain('markdown(content("lorem") + "\\n\\n" + "Lorem ipsum *dolor sit amet*');
    expect(script).toContain('{warn}');
    expect(script).not.toContain('{em}');
    expect(stylePreviewScript('ink', { ...DEFAULT_STYLE, example: 'Dolor' }, 1000, 'typeset')).toContain('markdown("Dolor")');
  });
});

describe('style settings', () => {
  it('keeps pressures in order', () => {
    expect(withSetting(DEFAULT_STYLE, 'min_pressure', 0.9).max_pressure).toBe(0.9);
    expect(withSetting(DEFAULT_STYLE, 'max_pressure', 0.1).min_pressure).toBe(0.1);
  });

  it('has italic and skew give way to each other', () => {
    const skewed = withSetting({ ...DEFAULT_STYLE, italic: true }, 'skew', 0.3);
    expect([skewed.italic, skewed.skew]).toEqual([false, 0.3]);
    const italic = withSetting(skewed, 'italic', true);
    expect([italic.italic, italic.skew]).toEqual([true, null]);
  });

  it('reads numbers and pairs however a file has them', () => {
    expect([numberOf(3), numberOf(true), numberOf(false), numberOf(null)]).toEqual([3, 2, null, null]);
    expect([pairOf(0.1), pairOf([0.1, 0.2])]).toEqual([[0.1, 0.1], [0.1, 0.2]]);
  });

  it('reads lines however a file has them, and writes them as briefly as it can', () => {
    expect(readRule(false)).toEqual({ count: 0, spacing: null, style: {} });
    expect(readRule(true)).toEqual({ count: 1, spacing: null, style: {} });
    expect(readRule(3)).toEqual({ count: 3, spacing: null, style: {} });
    expect(readRule({ spacing: 0.2 })).toEqual({ count: 1, spacing: 0.2, style: {} });
    expect(ruleValue({ count: 0, spacing: 0.2, style: {} })).toBe(false);
    expect(ruleValue({ count: 2, spacing: null, style: {} })).toBe(2);
    expect(ruleValue({ count: 2, spacing: null, style: { color: 'red' } })).toEqual({ count: 2, style: { color: 'red' } });
  });

  it('starts what a span sets with something the engine takes', () => {
    for (const field of SPAN_FIELDS) {
      expect(startingValue(field), field.key).not.toBeNull();
      expect(field.unset, field.key).toBeUndefined();
    }
    expect(isTypeSetting('underline')).toBe(true);
    expect(isTypeSetting('color')).toBe(false);
  });

  it('won\'t name a span for a setting', () => {
    expect(spanNameProblem('warn', [])).toBeNull();
    expect(spanNameProblem('passes', [])).toMatch(/setting's name/);
    expect(spanNameProblem('a b', [])).toMatch(/letters/);
    expect(spanNameProblem('warn', ['warn'])).toMatch(/already/);
  });
});
