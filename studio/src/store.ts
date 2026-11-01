// The document the studio shows: the bundled sample until the engineer opens an export with the
// file picker. The sample is a real `patchback fit` of the committed demo voice demos/pluck,
// exported by `patchback-studio export` (`task studio-fixtures` writes it, `task
// studio-fixtures-check` holds it to a fresh run). It goes through the same parser as a picked
// file.
import { shallowRef } from "vue";
import sampleText from "../fixtures/sample-run.json?raw";
import { DocumentError, parseDocument, type StudioDocument } from "./document";

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
