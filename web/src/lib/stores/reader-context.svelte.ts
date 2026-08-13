import { createContext } from "svelte";

import { reader as defaultReader, type ReaderApi } from "./reader.svelte";

const [useReaderCtx, setReaderCtx] = createContext<ReaderApi | undefined>();

export function setReaderContext(instance: ReaderApi = defaultReader): ReaderApi {
  setReaderCtx(instance);
  return instance;
}

export function useReader(): ReaderApi {
  return useReaderCtx() ?? defaultReader;
}
