import type { BrushFile, BrushNodeType, BrushPortDef, WireType } from '../scribe';
import {
  NodeTypes, addNode, connect, disconnect, fromDisplay, layout, parseWire, removeNode, setInput, toDisplay, toggleExposed, wireText, wiredInto,
} from './brush-model';
import { sampleCurve } from './curve-editor.component';

function port(name: string, dir: 'Input' | 'Output', wire_type: WireType, extra: Partial<BrushPortDef> = {}): BrushPortDef {
  return {
    name, dir, wire_type, label: name, description: '', value: 0, min: 0, max: 1, step: 0, unit_type: 'Normalized', enum_options: null,
    exposable: true, exposed: false, wirable: true, visible_when: null, natural_range: null, ...extra,
  } as BrushPortDef;
}

function type(type_id: string, ports: BrushPortDef[], is_terminal = false): BrushNodeType {
  return { type_id, category: 'math', display_name: type_id, description: '', ports, is_terminal };
}

const TYPES = new NodeTypes([
  type('pen_input', [port('pressure', 'Output', 'Scalar'), port('position', 'Output', 'Vec2')]),
  type('circle', [port('softness', 'Input', 'Scalar', { value: 0.1 }), port('algorithm', 'Input', 'Enum', { wirable: false }), port('mask', 'Output', 'Scalar')]),
  type('paint', [port('size', 'Input', 'Scalar', { value: 1, exposed: true }), port('position', 'Input', 'Vec2'), port('rgba', 'Input', 'Vec4')], true),
]);

const BRUSH: BrushFile = {
  name: 'Test',
  nodes: { pen_input: { type: 'pen_input' }, circle: { type: 'circle' }, paint: { type: 'paint' } },
  connections: ['pen_input.position -> paint.position'],
};

describe('brush model', () => {
  it('reads and writes wires as Darkly does', () => {
    const wire = parseWire('a.b -> c.d')!;
    expect(wire).toEqual({ from: 'a', fromPort: 'b', to: 'c', toPort: 'd' });
    expect(wireText(wire)).toBe('a.b -> c.d');
    expect(parseWire('nonsense')).toBeNull();
  });

  it('wires outputs to inputs of types they fit, one wire to an input', () => {
    const wired = connect(BRUSH, TYPES, { from: 'pen_input', fromPort: 'pressure', to: 'paint', toPort: 'size' });
    expect(typeof wired).not.toBe('string');
    const again = connect(wired as BrushFile, TYPES, { from: 'circle', fromPort: 'mask', to: 'paint', toPort: 'size' }) as BrushFile;
    expect(wiredInto(again, 'paint', 'size')?.from).toBe('circle');
    expect(again.connections?.length).toBe(2);
    expect(connect(BRUSH, TYPES, { from: 'pen_input', fromPort: 'position', to: 'paint', toPort: 'size' })).toContain('takes a number');
    expect(connect(BRUSH, TYPES, { from: 'pen_input', fromPort: 'pressure', to: 'circle', toPort: 'algorithm' })).toContain('not wired');
    expect(disconnect(again, 'paint', 'size').connections).toEqual(['pen_input.position -> paint.position']);
  });

  it('refuses loops', () => {
    const one = connect(BRUSH, TYPES, { from: 'circle', fromPort: 'mask', to: 'paint', toPort: 'size' }) as BrushFile;
    const { brush, id } = addNode(one, TYPES, 'circle');
    const two = connect(brush, TYPES, { from: id, fromPort: 'mask', to: 'circle', toPort: 'softness' }) as BrushFile;
    expect(connect(two, TYPES, { from: 'circle', fromPort: 'mask', to: id, toPort: 'softness' })).toBe('that would make a loop');
  });

  it('adds nodes with fresh ids and their exposed ports, and removes them with their wires', () => {
    const { brush, id } = addNode(BRUSH, TYPES, 'paint');
    expect(id).toBe('paint_2');
    expect(brush.exposed_ports).toEqual({ 'paint_2.size': {} });
    const gone = removeNode(brush, 'paint');
    expect(gone.nodes['paint']).toBeUndefined();
    expect(gone.connections).toBeUndefined();
  });

  it('keeps only inputs set otherwise than their defaults', () => {
    const set = setInput(BRUSH, TYPES, 'circle', 'softness', 0.5);
    expect(set.nodes['circle'].inputs).toEqual({ softness: 0.5 });
    expect(setInput(set, TYPES, 'circle', 'softness', 0.1).nodes['circle'].inputs).toBeUndefined();
  });

  it('exposes ports in the order they are put on the bar', () => {
    const one = toggleExposed(BRUSH, 'circle', 'softness');
    const two = toggleExposed(one, 'paint', 'size');
    expect(Object.keys(two.exposed_ports!)).toEqual(['circle.softness', 'paint.size']);
    expect(toggleExposed(toggleExposed(two, 'circle', 'softness'), 'paint', 'size').exposed_ports).toBeUndefined();
  });

  it('shows percentages and degrees as Darkly does', () => {
    expect(toDisplay('Percent', 0.25)).toBeCloseTo(25);
    expect(fromDisplay('Percent', 25)).toBeCloseTo(0.25);
    expect(toDisplay('Degrees', Math.PI)).toBeCloseTo(180);
    expect(fromDisplay('Degrees', 90)).toBeCloseTo(Math.PI / 2);
  });

  it('lays nodes out from the pen to the paint', () => {
    const wired = connect(BRUSH, TYPES, { from: 'circle', fromPort: 'mask', to: 'paint', toPort: 'size' }) as BrushFile;
    const at = layout(wired, TYPES);
    expect(at['pen_input'].x).toBeLessThan(at['paint'].x);
    expect(at['circle'].x).toBeLessThan(at['paint'].x);
  });

  it('samples a curve through its points', () => {
    const line = sampleCurve([[0, 0], [1, 1]], 4);
    expect(line.map(([, y]) => y)).toEqual([0, 0.25, 0.5, 0.75, 1]);
    const bent = sampleCurve([[0, 0], [0.5, 0.8], [1, 1]], 2);
    expect(bent[1][1]).toBeCloseTo(0.8);
  });
});
