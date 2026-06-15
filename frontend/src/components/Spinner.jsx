export default function Spinner({ size }) {
  const cls = size === 'lg' ? 'spinner spinner-lg' : 'spinner';
  return <div className={cls} />;
}
