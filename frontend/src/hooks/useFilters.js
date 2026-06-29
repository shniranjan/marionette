import { useState, useMemo, useEffect, useRef } from 'react';

/**
 * Generic hook for client-side filtering, searching, and sorting.
 *
 * @param {Array} data - The full dataset
 * @param {Object} config
 * @param {string[]} config.searchFields - Field names to search with text query
 * @param {string} [config.stateField] - Field name for state filtering (e.g., 'state')
 * @param {Object} [config.stateMap] - Maps filter values to matching field values
 *   e.g., { running: ['running'], stopped: ['exited','stopped'], paused: ['paused'] }
 * @returns {{ filtered, searchQuery, setSearchQuery, stateFilter, setStateFilter, sortKey, setSortKey, sortDir, setSortDir }}
 */
export default function useFilters(data, config = {}) {
  const { searchFields = [], stateField, stateMap } = config;

  // Raw input values
  const [searchQuery, setSearchQuery] = useState('');
  const [stateFilter, setStateFilter] = useState('all');
  const [sortKey, setSortKey] = useState(null);
  const [sortDir, setSortDir] = useState('asc');

  // 300ms debounced search query
  const [debouncedQuery, setDebouncedQuery] = useState('');
  const timerRef = useRef(null);

  useEffect(() => {
    if (timerRef.current) clearTimeout(timerRef.current);
    timerRef.current = setTimeout(() => {
      setDebouncedQuery(searchQuery);
    }, 300);
    return () => {
      if (timerRef.current) clearTimeout(timerRef.current);
    };
  }, [searchQuery]);

  // Reset sort when filter changes (sorted view of new results)
  const prevFilterRef = useRef({ query: '', state: 'all' });
  useEffect(() => {
    const prev = prevFilterRef.current;
    if (prev.query !== debouncedQuery || prev.state !== stateFilter) {
      setSortKey(null);
      setSortDir('asc');
    }
    prevFilterRef.current = { query: debouncedQuery, state: stateFilter };
  }, [debouncedQuery, stateFilter]);

  const filtered = useMemo(() => {
    // Guard: empty or non-array data
    if (!Array.isArray(data) || data.length === 0) return [];

    let result = data;

    // Text search (case-insensitive)
    if (debouncedQuery && searchFields.length > 0) {
      const q = debouncedQuery.toLowerCase();
      result = result.filter((item) =>
        searchFields.some((field) => {
          const val = item[field];
          // Treat null/undefined as empty string (no match)
          return val != null ? String(val).toLowerCase().includes(q) : false;
        }),
      );
    }

    // State filter
    if (stateFilter !== 'all' && stateField && stateMap) {
      const allowed = stateMap[stateFilter];
      if (allowed) {
        result = result.filter((item) => allowed.includes(item[stateField]));
      }
    }

    // Sort (null sortKey = unsorted, original order preserved)
    if (sortKey) {
      result = [...result].sort((a, b) => {
        const va = a[sortKey] ?? '';
        const vb = b[sortKey] ?? '';
        let cmp = 0;
        if (typeof va === 'number' && typeof vb === 'number') {
          cmp = va - vb;
        } else if (typeof va === 'string' && typeof vb === 'string') {
          cmp = va.localeCompare(vb);
        } else if (va < vb) {
          cmp = -1;
        } else if (va > vb) {
          cmp = 1;
        }
        return sortDir === 'desc' ? -cmp : cmp;
      });
    }

    return result;
  }, [data, debouncedQuery, searchFields, stateFilter, stateField, stateMap, sortKey, sortDir]);

  return {
    filtered,
    searchQuery,
    setSearchQuery,
    stateFilter,
    setStateFilter,
    sortKey,
    setSortKey,
    sortDir,
    setSortDir,
  };
}
