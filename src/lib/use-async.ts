import { useCallback, useEffect, useState } from "react";
import { errorText } from "./api";

/** Loads data on mount; `reload` refetches, `set` replaces it after a mutation. */
export function useAsync<T>(load: () => Promise<T>, deps: unknown[] = []) {
  const [data, setData] = useState<T | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  const reload = useCallback(async () => {
    setLoading(true);
    try {
      setData(await load());
      setError(null);
    } catch (e) {
      setError(errorText(e));
    } finally {
      setLoading(false);
    }
  }, deps);

  useEffect(() => {
    reload();
  }, [reload]);

  return { data, setData, error, setError, loading, reload };
}

/** Runs a mutation and reports its error; returns the result or undefined. */
export async function attempt<T>(fn: () => Promise<T>, onError: (msg: string | null) => void): Promise<T | undefined> {
  try {
    const result = await fn();
    onError(null);
    return result;
  } catch (e) {
    onError(errorText(e));
    return undefined;
  }
}
