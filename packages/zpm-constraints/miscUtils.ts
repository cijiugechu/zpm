export interface GetSetMap<K, V> {
  get(k: K): V | undefined;
  set(k: K, v: V): void;
}

export function getFactoryWithDefault<K, T>(map: GetSetMap<K, T>, key: K, factory: () => NoInfer<T>) {
  let value = map.get(key);

  if (typeof value === `undefined`)
    map.set(key, value = factory());

  return value;
}

export function getArrayWithDefault<K, T>(map: GetSetMap<K, Array<T>>, key: K) {
  let value = map.get(key);

  if (typeof value === `undefined`)
    map.set(key, value = []);

  return value;
}

export function getSetWithDefault<K, T>(map: GetSetMap<K, Set<T>>, key: K) {
  let value = map.get(key);

  if (typeof value === `undefined`)
    map.set(key, value = new Set<T>());

  return value;
}

export function getMapWithDefault<K, MK, MV>(map: GetSetMap<K, Map<MK, MV>>, key: K) {
  let value = map.get(key);

  if (typeof value === `undefined`)
    map.set(key, value = new Map<MK, MV>());

  return value;
}

export function accessArrayWithDefault<K extends string | number | symbol, V>(map: Record<K, Array<V>>, key: K) {
  const value = Object.hasOwn(map, key)
    ? map[key]
    : map[key] = [];

  return value;
}

export function accessRecordWithDefault<K extends string | number | symbol, V>(map: Record<K, Record<string, V>>, key: K) {
  const value = Object.hasOwn(map, key)
    ? map[key]
    : map[key] = {};

  return value;
}

export function accessFactoryWithDefault<K extends string | number | symbol, T>(map: Record<K, T>, key: K, factory: () => T) {
  const value = Object.hasOwn(map, key)
    ? map[key]
    : map[key] = factory();

  return value;
}
