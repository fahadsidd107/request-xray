import createClient from "openapi-fetch";
import type { paths } from "./schema.js";

export type { components, operations, paths } from "./schema.js";

export function createQueryClient(baseUrl: string, fetch?: typeof globalThis.fetch) {
  return createClient<paths>({ baseUrl, fetch });
}
