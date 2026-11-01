import { DEFAULT_STYLE, styleSource } from './style-builder';

describe('style builder', () => {
  it('writes a colour as a string, for its option', () => {
    expect(styleSource('ink', DEFAULT_STYLE)).toContain('#{ size: 26.0, color: "#2b1d12" }');
  });

  it('writes a gradient as new_gradient, for the same option', () => {
    const source = styleSource('ink', { ...DEFAULT_STYLE, color: { colors: ['#9b2318', [0.6, '#9b2318'], '#2b1d12'], vertical: true } });
    expect(source).toContain('color: new_gradient(["#9b2318", [0.6, "#9b2318"], "#2b1d12"], #{ vertical: true }) }');
    expect(source).toContain('new_brush_style(o.color, "Ink Pen", 1.4,');
  });
});
