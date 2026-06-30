import YamlEditor from './YamlEditor';

export default function ComposeEditor({ label, value, onChange, readOnly }) {
  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: '6px', width: '100%' }}>
      {label && (
        <label style={{
          fontSize: '0.85rem',
          fontWeight: 600,
          color: 'var(--text-primary)',
        }}>
          {label}
        </label>
      )}
      <YamlEditor
        value={value}
        onChange={onChange}
        readOnly={readOnly}
        showLineNumbers
        fill
      />
    </div>
  );
}
