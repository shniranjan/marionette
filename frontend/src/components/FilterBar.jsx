export default function FilterBar({
  searchQuery,
  onSearchChange,
  searchPlaceholder = 'Search...',
  stateFilter,
  onStateFilterChange,
  stateOptions,
  filteredCount,
  totalCount,
}) {
  const hasChips = stateOptions && stateOptions.length > 0 && onStateFilterChange;

  const countLabel =
    filteredCount === totalCount
      ? `${totalCount} total`
      : `Showing ${filteredCount} of ${totalCount}`;

  return (
    <div
      style={{
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'space-between',
        gap: '12px',
        flexWrap: 'wrap',
        marginBottom: '12px',
      }}
    >
      <div style={{ flex: '1 1 240px', maxWidth: '400px' }}>
        <input
          type="search"
          placeholder={searchPlaceholder}
          value={searchQuery}
          onChange={(e) => onSearchChange(e.target.value)}
          aria-label="Search"
        />
      </div>

      {hasChips && (
        <div className="btn-group" role="group" aria-label="Filter by state">
          {stateOptions.map((opt) => (
            <button
              key={opt.value}
              type="button"
              className={stateFilter === opt.value ? 'btn-primary' : 'outline'}
              style={{ fontSize: '0.8rem', padding: '4px 10px' }}
              onClick={() =>
                onStateFilterChange(stateFilter === opt.value ? null : opt.value)
              }
            >
              {opt.label}
              {opt.count !== undefined && (
                <span style={{ marginLeft: '4px', opacity: 0.7, fontSize: '0.75em' }}>
                  {opt.count}
                </span>
              )}
            </button>
          ))}
        </div>
      )}

      {totalCount !== undefined && (
        <span
          style={{
            fontSize: '0.8rem',
            color: 'var(--pico-muted-color, #8b949e)',
            whiteSpace: 'nowrap',
          }}
        >
          {countLabel}
        </span>
      )}
    </div>
  );
}
