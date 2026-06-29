import { useState, useCallback, useEffect } from 'react';

const STORAGE_KEY = 'marionette_favorites';

function load() {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw);
    return Array.isArray(parsed) ? parsed : [];
  } catch {
    return [];
  }
}

function save(items) {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(items));
  } catch {
    // localStorage full or unavailable — silently ignore
  }
}

/**
 * Hook for managing pinned/favorite containers in localStorage.
 * Returns [favorites, { addFavorite, removeFavorite, isFavorite, toggleFavorite }]
 */
export default function useFavorites() {
  const [favorites, setFavorites] = useState(load);

  // Listen for storage changes from other tabs
  useEffect(() => {
    const handler = (e) => {
      if (e.key === STORAGE_KEY) {
        setFavorites(load());
      }
    };
    window.addEventListener('storage', handler);
    return () => window.removeEventListener('storage', handler);
  }, []);

  const addFavorite = useCallback((id, name) => {
    setFavorites((prev) => {
      if (prev.some((f) => f.id === id)) return prev;
      const next = [...prev, { id, name }];
      save(next);
      return next;
    });
  }, []);

  const removeFavorite = useCallback((id) => {
    setFavorites((prev) => {
      const next = prev.filter((f) => f.id !== id);
      save(next);
      return next;
    });
  }, []);

  const isFavorite = useCallback(
    (id) => favorites.some((f) => f.id === id),
    [favorites],
  );

  const toggleFavorite = useCallback(
    (id, name) => {
      if (isFavorite(id)) {
        removeFavorite(id);
      } else {
        addFavorite(id, name);
      }
    },
    [isFavorite, addFavorite, removeFavorite],
  );

  return {
    favorites,
    addFavorite,
    removeFavorite,
    isFavorite,
    toggleFavorite,
  };
}
