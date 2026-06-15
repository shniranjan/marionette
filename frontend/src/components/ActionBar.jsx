import { api } from '../api/client';

export default function ActionBar({ containerId, state, onAction }) {
  const isRunning = state === 'running';
  const isPaused = state === 'paused';
  const isStopped = state === 'stopped' || state === 'exited';

  const doAction = async (action) => {
    try {
      await api.post(`/api/containers/${containerId}/${action}`);
      if (onAction) onAction();
    } catch (err) {
      console.error(`Action ${action} failed:`, err);
    }
  };

  return (
    <div className="btn-group">
      {isStopped && (
        <button className="btn-success btn-sm" onClick={() => doAction('start')}>
          ▶ Start
        </button>
      )}
      {isRunning && (
        <>
          <button className="btn-warning btn-sm" onClick={() => doAction('stop')}>
            ⏹ Stop
          </button>
          <button className="btn-sm" onClick={() => doAction('restart')}>
            ↻ Restart
          </button>
          <button className="btn-sm" onClick={() => doAction('pause')}>
            ⏸ Pause
          </button>
        </>
      )}
      {isPaused && (
        <button className="btn-sm" onClick={() => doAction('unpause')}>
          ▶ Unpause
        </button>
      )}
      {(isRunning || isPaused) && (
        <button className="btn-sm" onClick={() => doAction('kill')}>
          ⚡ Kill
        </button>
      )}
      {isStopped && (
        <button className="btn-danger btn-sm" onClick={() => doAction('remove')}>
          🗑 Remove
        </button>
      )}
    </div>
  );
}
