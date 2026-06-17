import { Component } from 'react';

export default class ErrorBoundary extends Component {
  constructor(props) {
    super(props);
    this.state = { error: null };
  }

  static getDerivedStateFromError(error) {
    return { error };
  }

  render() {
    if (this.state.error) {
      return (
        <div style={{ padding: '48px', textAlign: 'center' }}>
          <div style={{ fontSize: '2rem', marginBottom: '16px' }}>⚠</div>
          <h2>Something went wrong</h2>
          <p className="text-secondary" style={{ marginTop: '8px', marginBottom: '16px' }}>
            {this.state.error.message || 'An unexpected error occurred'}
          </p>
          <button className="btn-primary" onClick={() => this.setState({ error: null })}>
            Try Again
          </button>
          <details style={{ marginTop: '24px', textAlign: 'left', maxWidth: '600px', marginLeft: 'auto', marginRight: 'auto' }}>
            <summary className="text-secondary" style={{ cursor: 'pointer', marginBottom: '8px' }}>Stack trace</summary>
            <pre className="log-output" style={{ fontSize: '0.75rem', background: 'var(--bg-tertiary)', padding: '12px', borderRadius: '6px', overflow: 'auto' }}>
              {this.state.error.stack || 'No stack trace'}
            </pre>
          </details>
        </div>
      );
    }
    return this.props.children;
  }
}
