// The document the studio shows: the bundled sample until the engineer opens an export with the
// file picker. The sample is a real `patchback fit` of the committed demo voice demos/pluck,
// exported by `patchback-studio export` (`task studio-fixtures` writes it, `task
// studio-fixtures-check` holds it to a fresh run). It goes through the same parser as a picked
// file.
// The sample is a chunk of its own, loaded by a dynamic import before the store is ready, so the
// application chunk does not carry its 390 kB.
import { shallowRef } from "vue";
import { DocumentError, parseDocument, type StudioDocument } from "./document";

const sampleText: string = (await import("../fixtures/sample-run.json?raw")).default;

export const SAMPLE_LABEL = "bundled sample: patchback fit of demos/pluck";

export interface Loaded {
  label: string;
  document: StudioDocument;
}

export const sample: Loaded = { label: SAMPLE_LABEL, document: parseDocument(sampleText) };

export const loaded = shallowRef<Loaded>(sample);

/** The refusal of the last file the engineer opened, until one loads. */
export const loadError = shallowRef<string | null>(null);

/** Loads `text`, the content of the file `label`; a refused file leaves the shown one in place. */
export function load(label: string, text: string): boolean {
  try {
    loaded.value = { label, document: parseDocument(text) };
    loadError.value = null;
    return true;
  } catch (error) {
    loadError.value = `${label}: ${error instanceof DocumentError ? error.message : String(error)}`;
    return false;
  }
}

export async function open(file: File): Promise<boolean> {
  return load(file.name, await file.text());
}
