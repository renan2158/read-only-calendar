import { invoke } from "@tauri-apps/api/core";
export const native = () => "__TAURI_INTERNALS__" in window;
export async function call<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  if (!native())
    throw new Error(
      "Open the installed desktop application to access your encrypted vault.",
    );
  return invoke<T>(command, args);
}
export const message = (e: unknown) =>
  typeof e === "string"
    ? e
    : e instanceof Error
      ? e.message
      : "The operation could not complete.";
