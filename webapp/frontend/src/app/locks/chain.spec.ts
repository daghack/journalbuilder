import shiftVigenere from '../../../generated/fixtures/shift-vigenere';
import type { LockPorts } from '../scribe';
import {
  connect, openInputs, openOutputs, removeNode, renameNode, symbolsText, textSymbols, wireProblem, type ChainModel, type PortsByNode,
} from './chain-model';
import { chainSource, exampleProblem, journalChain } from './chain-source';

const ports = (inputs: [string, string][], outputs: [string, string][]): LockPorts => ({ inputs, outputs, options: {} });
const B27 = 'Base27Symbol';

/** webapp/locks/shift_vigenere.rhai, as the editor holds it. */
function shiftVigenereModel(): { model: ChainModel; ports: PortsByNode } {
  return {
    model: {
      name: 'shift_vigenere',
      description: 'the key shifted along the alphabet, then used as the key of a Vigenère cipher. A composite lock: shift_cipher wired into vigenere_cipher.',
      options: [{ name: 'shift', kind: 'integer', default: 3 }],
      nodes: [
        { id: 'shift', type: 'shift_cipher', options: { base: 27 }, promoted: { shift: 'shift' }, x: 120, y: 60 },
        { id: 'vig', type: 'vigenere_cipher', options: { cipher_base: 27, key_base: 27, plain_base: 27 }, promoted: {}, x: 480, y: 60 },
      ],
      wires: [{ from: 'shift.cipher_text', to: 'vig.key' }],
      fixed: {},
      inputNames: { 'shift.plain_text': 'key', 'vig.plain_text': 'plain_text' },
      outputNames: { 'vig.key': 'shifted_key' },
      test: { key: { text: 'KEY' }, plain_text: { text: 'ATTACK AT DAWN' } },
    },
    ports: {
      shift: ports([['plain_text', B27]], [['cipher_text', B27]]),
      vig: ports([['key', B27], ['plain_text', B27]], [['cipher_text', B27], ['key', B27]]),
    },
  };
}

const kinds = (type: string) => (type === 'shift_cipher' ? { base: 'integer', shift: 'integer' } : { cipher_base: 'integer', key_base: 'integer', plain_base: 'integer' }) as Record<string, 'integer'>;

describe('composite lock files', () => {
  it('writes webapp/locks/shift_vigenere.rhai as it is', () => {
    const { model, ports } = shiftVigenereModel();
    const outputs = { cipher_text: { text: 'OAUOKLNIUNLBJV' }, shifted_key: { text: 'NHA' } };
    expect(chainSource(model, { ports, optionKinds: kinds, outputs })).toBe(shiftVigenere);
  });

  it('leaves examples out when the test case is incomplete', () => {
    const { model, ports } = shiftVigenereModel();
    const partial = { ...model, test: { key: { text: 'KEY' } } };
    expect(exampleProblem(partial, ports)).toBe("the test has no value for 'plain_text'");
    expect(chainSource(partial, { ports, optionKinds: kinds })).not.toContain('fn examples()');
  });

  it('writes the chain as journal script', () => {
    const { model, ports } = shiftVigenereModel();
    expect(journalChain(model, { ports, optionKinds: kinds })).toBe([
      'let lock = new_lock("shift_vigenere")',
      '\t.add("shift", "shift_cipher", #{ base: 27, shift: 3 })',
      '\t.add("vig", "vigenere_cipher", #{ cipher_base: 27, key_base: 27, plain_base: 27 })',
      '\t.connect("shift.cipher_text", "vig.key");',
      'let result = lock.run(#{ "shift.plain_text": symbols(27, "KEY"), "vig.plain_text": symbols(27, "ATTACK AT DAWN") });',
      '',
    ].join('\n'));
  });
});

describe('chains', () => {
  it('names open ports by their own names, unless two share one', () => {
    const { model, ports } = shiftVigenereModel();
    const plain = { ...model, inputNames: {}, outputNames: {} };
    expect(openInputs(plain, ports).map((p) => p.name)).toEqual(['shift_plain_text', 'vig_plain_text']);
    // vig.key passes its input through: an output may share the input's name only as the same port.
    expect(openOutputs(plain, ports).map((p) => [p.port, p.name])).toEqual([['vig.cipher_text', 'cipher_text'], ['vig.key', 'key']]);
  });

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
    expect(renamed.inputNames).toEqual({ 'turn.plain_text': 'key', 'vig.plain_text': 'plain_text' });
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
