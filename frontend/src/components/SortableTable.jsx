import { useState, useMemo } from 'react';

export default function SortableTable({
  data,
  columns,
  keyField = 'id',
  onRowClick,
  selected,
  onToggle,
  onToggleAll,
  allSelected,
  emptyMessage = 'No items',
  fav,
}) {
  const [sortKey, setSortKey] = useState(null);
  const [sortDir, setSortDir] = useState(null);

  const sorted = useMemo(() => {
    if (!data) return [];
    const d = [...data];
    if (sortKey && sortDir) {
      d.sort((a, b) => {
        const va = a[sortKey] ?? '';
        const vb = b[sortKey] ?? '';
        const cmp = typeof va === 'number' ? va - vb : String(va).localeCompare(String(vb));
        return sortDir === 'asc' ? cmp : -cmp;
      });
    }
    return d;
  }, [data, sortKey, sortDir]);

  const handleSort = (key) => {
    if (sortKey !== key) {
      setSortKey(key);
      setSortDir('asc');
    } else if (sortDir === null) {
      setSortDir('asc');
    } else if (sortDir === 'asc') {
      setSortDir('desc');
    } else {
      setSortKey(null);
      setSortDir(null);
    }
  };

  const sortIndicator = (key) => {
    if (sortKey !== key) return null;
    if (sortDir === 'asc') return <span style={{ fontSize: '0.75em' }}> ▲</span>;
    if (sortDir === 'desc') return <span style={{ fontSize: '0.75em' }}> ▼</span>;
    return null;
  };

  const hasCheckboxes = selected !== undefined && onToggle !== undefined;
  const hasFav = fav !== undefined && fav.isFavorite !== undefined;

  if (!data || data.length === 0) {
    return (
      <div style={{ padding: '24px', textAlign: 'center', color: 'var(--pico-muted-color, #8b949e)' }}>
        {emptyMessage}
      </div>
    );
  }

  return (
    <div style={{ overflowX: 'auto', maxWidth: '100%', WebkitOverflowScrolling: 'touch' }}>
      <table style={{ fontSize: '0.8rem', lineHeight: 1.3, whiteSpace: 'nowrap' }}>
        <thead>
          <tr>
            {hasCheckboxes && (
              <th style={{ width: '1%', padding: '4px 6px' }}>
                {onToggleAll && (
                  <input
                    type="checkbox"
                    checked={allSelected || false}
                    onChange={onToggleAll}
                    style={{ margin: 0 }}
                  />
                )}
              </th>
            )}
            {hasFav && (
              <th style={{ width: '1%', padding: '4px 2px' }}></th>
            )}
            {columns.map((col) => (
              <th
                key={col.key}
                onClick={() => col.sortable !== false && handleSort(col.key)}
                style={{
                  cursor: col.sortable !== false ? 'pointer' : 'default',
                  userSelect: 'none',
                  whiteSpace: 'nowrap',
                  padding: '4px 8px',
                  fontSize: '0.75rem',
                  fontWeight: 600,
                  ...(col.width ? { width: col.width } : {}),
                }}
              >
                {col.label}
                {sortIndicator(col.key)}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {sorted.map((row) => (
            <tr
              key={row[keyField]}
              onClick={() => onRowClick && onRowClick(row)}
              style={{ cursor: onRowClick ? 'pointer' : 'default' }}
            >
              {hasCheckboxes && (
                <td onClick={(e) => e.stopPropagation()} style={{ padding: '3px 6px' }}>
                  <input
                    type="checkbox"
                    checked={selected.has(row[keyField])}
                    onChange={() => onToggle(row[keyField])}
                    style={{ margin: 0 }}
                  />
                </td>
              )}
              {hasFav && (
                <td onClick={(e) => e.stopPropagation()} style={{ padding: '3px 2px' }}>
                  <button
                    onClick={() => fav.onToggle(row[keyField], row.name)}
                    title={fav.isFavorite(row[keyField]) ? 'Unpin' : 'Pin to favorites'}
                    style={{
                      border: 'none',
                      background: 'none',
                      cursor: 'pointer',
                      padding: '1px 3px',
                      fontSize: '0.9rem',
                      lineHeight: 1,
                      color: fav.isFavorite(row[keyField]) ? '#f0c040' : 'var(--text-secondary)',
                      opacity: fav.isFavorite(row[keyField]) ? 1 : 0.35,
                      transition: 'opacity 0.15s, color 0.15s',
                    }}
                    onMouseEnter={(e) => { e.currentTarget.style.opacity = '1'; }}
                    onMouseLeave={(e) => {
                      e.currentTarget.style.opacity = fav.isFavorite(row[keyField]) ? '1' : '0.35';
                    }}
                  >
                    {fav.isFavorite(row[keyField]) ? '★' : '☆'}
                  </button>
                </td>
              )}
              {columns.map((col) => (
                <td key={col.key} style={{
                  padding: '3px 8px',
                  maxWidth: col.maxWidth || '240px',
                  overflow: 'hidden',
                  textOverflow: 'ellipsis',
                  whiteSpace: 'nowrap',
                }}>
                  {col.render ? col.render(row[col.key], row) : row[col.key]}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
