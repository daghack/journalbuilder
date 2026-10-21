import ravenSky from '../../../generated/fixtures/raven-sky';
import type { LockPorts } from '../scribe';
import {
  connect, connectRenderer, emptyChain, openInputs, openOutputs, removeNode, renameNode, rendererInputs, symbolsText, textSymbols, wireProblem, type ChainModel, type PortsByNode,
} from './chain-model';
import { chainSource, hasExample, journalChain, journalSnippet } from './chain-source';

const ports = (inputs: [string, string][], outputs: [string, string][]): LockPorts => ({ inputs, outputs, options: {} });
const B27 = 'Base27Symbol';
const TEXT = 'ONCE UPON A MIDNIGHT DREARY WHILE I PONDERED WEAK AND WEARY';
const CODE = 'huffman::code(symbols(27, "THE QUICK BROWN FOX"), 27, 3)';

/** webapp/chains/raven_sky.rhai, as the editor holds it. */
function ravenModel(): { model: ChainModel; ports: PortsByNode } {
  const option = (name: string, value: number) => ({ name, kind: 'integer' as const, default: value });
  return {
    model: {
      name: 'raven_sky',
      description: ravenSky.split('\n').filter((l) => l.startsWith('//')).slice(0, 6).map((l) => l.replace(/^\/\/ ?/, '')).join(' ').replace(/^raven_sky: /, ''),
      options: [option('most_stars', 9), option('name_seed', 11), option('output_base', 3), option('seed', 7), option('size_seed', 29)],
      nodes: [
        { id: 'enc', type: 'huffman_encode', options: { input_base: 27 }, promoted: { output_base: 'output_base' }, x: 120, y: 60 },
        { id: 'msg', type: 'huffman_apply', options: { input_base: 27 }, promoted: { output_base: 'output_base' }, x: 480, y: 300 },
        {
          id: 'sky', type: 'star_chart', options: { input_base: 27 },
          promoted: { most_stars: 'most_stars', name_seed: 'name_seed', output_base: 'output_base', seed: 'seed', size_seed: 'size_seed' }, x: 480, y: 60,
        },
      ],
      wires: [{ from: 'enc.code', to: 'sky.code' }, { from: 'enc.code', to: 'msg.code' }],
      renderers: [
        { id: 'message', type: 'trit_message', options: {}, promoted: { base: 'output_base' }, x: 840, y: 300 },
        { id: 'pages', type: 'star_pages', options: {}, promoted: {}, x: 840, y: 60 },
      ],
      renderWires: [{ from: 'msg.cipher_text', to: 'message.message' }, { from: 'sky.chart', to: 'pages.chart' }],
      fixed: {},
      test: { 'enc.plain_text': { text: TEXT }, 'msg.plain_text': { text: 'NEVERMORE' } },
    },
    ports: {
      enc: ports([['plain_text', B27]], [['cipher_text', 'Base3Symbol'], ['code', 'HuffmanCode']]),
      msg: ports([['code', 'HuffmanCode'], ['plain_text', B27]], [['cipher_text', 'Base3Symbol'], ['code', 'HuffmanCode']]),
      sky: ports([['code', 'HuffmanCode']], [['chart', 'StarChart']]),
      message: ports([['message', 'Base3Symbol']], []),
      pages: ports([['chart', 'StarChart']], []),
    },
  };
}

const kinds = () => ({ input_base: 'integer', output_base: 'integer' }) as Record<string, 'integer'>;

describe('chain files', () => {
  it('writes webapp/chains/raven_sky.rhai as it is', () => {
    const { model, ports } = ravenModel();
    expect(chainSource(model, { ports, optionKinds: kinds })).toBe(ravenSky);
  });

  it('names the chain\'s values by their ports', () => {
    const { model, ports } = ravenModel();
    expect(openInputs(model, ports).map((p) => p.port)).toEqual(['enc.plain_text', 'msg.plain_text']);
    // An output a renderer draws stays the chain's.
    expect(openOutputs(model, ports).map((p) => p.port)).toEqual(['enc.cipher_text', 'msg.cipher_text', 'msg.code', 'sky.chart']);
    expect(rendererInputs(model, 'pages')).toEqual({ chart: 'sky.chart' });
  });

  it('leaves example() out until every input has a test value', () => {
    const { model, ports } = ravenModel();
    const partial = { ...model, test: { 'msg.plain_text': { text: 'NEVERMORE' } } };
    expect(hasExample(partial, ports)).toBe(false);
    expect(chainSource(partial, { ports, optionKinds: kinds })).not.toContain('fn example()');
  });

  it('keeps a fixed value kind in the chain itself', () => {
    const { model, ports } = ravenModel();
    const fixed = { ...model, wires: model.wires.filter((w) => w.to !== 'msg.code'), fixed: { 'msg.code': { rhai: CODE } } };
    expect(chainSource(fixed, { ports, optionKinds: kinds })).toContain(`fixed: #{ "msg.code": ${CODE} },`);
  });

  it('writes journal script using the chain, or building it lock by lock', () => {
    const { model, ports } = ravenModel();
    expect(journalSnippet(model, ports)).toBe([
      'let c = chain("raven_sky", #{ most_stars: 9, name_seed: 11, output_base: 3, seed: 7, size_seed: 29 });',
      `let result = c.run(#{ "enc.plain_text": symbols(27, "${TEXT}"), "msg.plain_text": symbols(27, "NEVERMORE") });`,
      '// result.get("enc.cipher_text"): Base3Symbol',
      '// result.get("msg.cipher_text"): Base3Symbol',
      '// result.get("msg.code"): HuffmanCode',
      '// result.get("sky.chart"): StarChart',
      'let message = result.canvases("message");  // what trit_message draws, an array of canvases',
      'let pages = result.canvases("pages");  // what star_pages draws, an array of canvases',
      '',
    ].join('\n'));
    const script = journalChain(model, { ports, optionKinds: kinds });
    expect(script).toContain('\t.add("enc", "huffman_encode", #{ input_base: 27, output_base: 3 })');
    expect(script).toContain('\t.attach("message", "trit_message", #{ base: 3 })');
    expect(script).toContain('\t.connect("sky.chart", "pages.chart");');
    expect(script).toContain('let pages = result.canvases("pages");');
  });
});

describe('renderers', () => {
  /** maze_lock with maze_glyphs attached to its maze. */
  function mazeModel(): { model: ChainModel; ports: PortsByNode } {
    return {
      model: {
        ...emptyChain(),
        name: 'drawn_maze',
        nodes: [{ id: 'ml', type: 'maze_lock', options: { input_base: 27 }, promoted: {}, x: 0, y: 0 }],
        renderers: [{ id: 'drawn', type: 'maze_glyphs', options: { cell: 30 }, promoted: {}, x: 0, y: 200 }],
        renderWires: [{ from: 'ml.generated_maze', to: 'drawn.maze' }],
        test: { 'ml.maze_text': { text: 'HELLO' }, 'ml.random_seed': { text: '4' } },
      },
      ports: {
        ml: ports([['maze_text', B27], ['random_seed', 'Base10Symbol']], [['generated_maze', 'Maze'], ['random_seed', 'Base10Symbol'], ['text_length', 'LockProcessInfo']]),
        drawn: ports([['maze', 'Maze']], []),
      },
    };
  }
  const mazeKinds = () => ({ input_base: 'integer', strategy: 'one_of' }) as Record<string, 'integer' | 'one_of'>;

  it('saves renderers with the ports they draw', () => {
    const { model, ports } = mazeModel();
    const source = chainSource(model, { ports, optionKinds: mazeKinds });
    expect(source).toContain('\t\trenderers: #{\n\t\t\tdrawn: #{ type: "maze_glyphs", options: #{ cell: 30 }, inputs: #{ maze: "ml.generated_maze" } },');
    expect(source).toContain('drawn: [0, 200]');
  });

  it('writes attach and connect into journal script, and the canvases', () => {
    const { model, ports } = mazeModel();
    const script = journalChain(model, { ports, optionKinds: mazeKinds });
    expect(script).toContain('\t.attach("drawn", "maze_glyphs", #{ cell: 30 })');
    expect(script).toContain('\t.connect("ml.generated_maze", "drawn.maze");');
    expect(script).toContain('let drawn = result.canvases("drawn");');
  });

  it('wires renderers only from outputs, by type, one wire an input', () => {
    const { model, ports } = mazeModel();
    expect(wireProblem(model, ports, 'ml.text_length', 'drawn.maze')).toBe('ml.text_length gives LockProcessInfo, but drawn.maze takes Maze');
    expect(wireProblem(model, ports, 'drawn.maze', 'ml.maze_text')).toBe('a renderer has no outputs to wire');
    const rewired = connectRenderer(model, 'ml.generated_maze', 'drawn.maze');
    expect(rewired.renderWires).toEqual([{ from: 'ml.generated_maze', to: 'drawn.maze' }]);
  });

  it('renames and removes renderers, and a lock takes its renderer wires with it', () => {
    const { model } = mazeModel();
    const renamed = renameNode(renameNode(model, 'drawn', 'grid'), 'ml', 'maze');
    expect(renamed.renderers[0].id).toBe('grid');
    expect(renamed.renderWires).toEqual([{ from: 'maze.generated_maze', to: 'grid.maze' }]);
    expect(removeNode(model, 'ml').renderWires).toEqual([]);
    expect(removeNode(model, 'drawn').renderers).toEqual([]);
  });
});

/** shift_cipher's key into vigenere_cipher. */
function shiftVigenereModel(): { model: ChainModel; ports: PortsByNode } {
  return {
    model: {
      ...emptyChain('shifted'),
      options: [{ name: 'shift', kind: 'integer', default: 3 }],
      nodes: [
        { id: 'shift', type: 'shift_cipher', options: { base: 27 }, promoted: { shift: 'shift' }, x: 120, y: 60 },
        { id: 'vig', type: 'vigenere_cipher', options: { cipher_base: 27, key_base: 27, plain_base: 27 }, promoted: {}, x: 480, y: 60 },
      ],
      wires: [{ from: 'shift.cipher_text', to: 'vig.key' }],
      test: { 'shift.plain_text': { text: 'KEY' }, 'vig.plain_text': { text: 'ATTACK AT DAWN' } },
    },
    ports: {
      shift: ports([['plain_text', B27]], [['cipher_text', B27]]),
      vig: ports([['key', B27], ['plain_text', B27]], [['cipher_text', B27], ['key', B27]]),
    },
  };
}

describe('chains', () => {
  it('hides an output passing a fixed input through', () => {
    const { model, ports } = shiftVigenereModel();
    const fixed = { ...removeNode(model, 'shift'), fixed: { 'vig.key': { text: 'KEY' } } };
    expect(openInputs(fixed, ports).map((p) => p.port)).toEqual(['vig.plain_text']);
    expect(openOutputs(fixed, ports).map((p) => p.port)).toEqual(['vig.cipher_text']);
  });

  it('wires only matching types, one wire to an input, and no loops', () => {
    const { model, ports } = shiftVigenereModel();
    const withRing: PortsByNode = { ...ports, ring: { inputs: [['plain_text', 'Base26Symbol']], outputs: [['cipher_text', 'Base26Symbol']], options: {} } };
    const ringed = { ...model, nodes: [...model.nodes, { id: 'ring', type: 'doublering', options: {}, promoted: {}, x: 0, y: 0 }] };
    expect(wireProblem(ringed, withRing, 'ring.cipher_text', 'vig.plain_text')).toBe('ring.cipher_text gives Base26Symbol, but vig.plain_text takes Base27Symbol');
    expect(wireProblem(model, ports, 'vig.cipher_text', 'shift.plain_text')).toBe('that would feed shift back into itself');
    expect(wireProblem(model, ports, 'vig.cipher_text', 'vig.plain_text')).toBe('that would feed vig back into itself');
    expect(wireProblem(model, ports, 'shift.cipher_text', 'vig.plain_text')).toBeNull();
    const rewired = connect(model, 'shift.cipher_text', 'vig.key');
    expect(rewired.wires).toEqual([{ from: 'shift.cipher_text', to: 'vig.key' }]);
  });

  it('renames a lock everywhere it appears', () => {
    const { model } = shiftVigenereModel();
    const renamed = renameNode(model, 'shift', 'turn');
    expect(renamed.wires).toEqual([{ from: 'turn.cipher_text', to: 'vig.key' }]);
    expect(renamed.test).toEqual({ 'turn.plain_text': { text: 'KEY' }, 'vig.plain_text': { text: 'ATTACK AT DAWN' } });
    expect(removeNode(model, 'vig').wires).toEqual([]);
  });

  it('reads and writes symbols as lockbuilder does', () => {
    expect(textSymbols(27, 'Hi z')).toEqual([8, 9, 0, 26]);
    expect(textSymbols(10, '1084')).toEqual([1, 0, 8, 4]);
    expect(textSymbols(26, 'baz')).toEqual([1, 0, 25]);
    expect(() => textSymbols(27, 'a,b')).toThrow("',' isn't a base-27 symbol");
    expect(symbolsText(27, [8, 9, 0, 26])).toBe('HI Z');
    expect(symbolsText(26, [1, 0, 25])).toBe('BAZ');
  });
});
