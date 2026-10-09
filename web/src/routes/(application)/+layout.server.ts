export async function load(): Promise<{ year: number }> {
  return { year: new Date().getFullYear() };
}
