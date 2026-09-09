import { useState, useEffect } from 'react';

interface EscrowEvent {
  id: number;
  event_type: string;
  tx_hash: string;
  ledger: number;
  timestamp: number;
  data: string;
}

interface EscrowState {
  escrowId: number;
  status: string;
  funder: string;
  receiver: string;
  totalDeposited: string;
  totalReleased: string;
  milestones: {
    description: string;
    status: string;
    amount: string;
  }[];
}

const API_BASE = import.meta.env.VITE_API_BASE || 'http://localhost:3001';

export function EscrowViewer() {
  const [escrowId, setEscrowId] = useState('');
  const [escrow, setEscrow] = useState<EscrowState | null>(null);
  const [events, setEvents] = useState<EscrowEvent[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');

  const fetchEscrow = async () => {
    if (!escrowId) return;
    setLoading(true);
    setError('');
    try {
      const res = await fetch(`${API_BASE}/api/escrow/${escrowId}`);
      const data = await res.json();
      if (data.success) {
        setEscrow(data.data);
      } else {
        setError(data.error || 'Escrow not found');
      }
    } catch (err) {
      setError('Failed to fetch escrow');
    } finally {
      setLoading(false);
    }
  };

  const fetchEvents = async () => {
    if (!escrowId) return;
    try {
      const res = await fetch(`${API_BASE}/api/gateway/events?contract=escrow&limit=50`);
      const data = await res.json();
      if (data.success) {
        setEvents(data.data);
      }
    } catch (err) {
      console.error('Failed to fetch events');
    }
  };

  useEffect(() => {
    if (escrowId) {
      fetchEscrow();
      fetchEvents();
    }
  }, [escrowId]);

  const statusColor = (status: string) => {
    switch (status) {
      case 'created': return '#6b7280';
      case 'funded': return '#3b82f6';
      case 'active': return '#10b981';
      case 'completed': return '#059669';
      case 'disputed': return '#ef4444';
      case 'released': return '#8b5cf6';
      case 'refunded': return '#f59e0b';
      default: return '#6b7280';
    }
  };

  return (
    <div style={{ maxWidth: 800, margin: '0 auto', padding: 24, fontFamily: 'system-ui' }}>
      <h1 style={{ fontSize: 24, fontWeight: 'bold', marginBottom: 16 }}>
        Escrow Viewer
      </h1>

      <div style={{ display: 'flex', gap: 8, marginBottom: 24 }}>
        <input
          type="text"
          placeholder="Enter Escrow ID"
          value={escrowId}
          onChange={(e) => setEscrowId(e.target.value)}
          style={{
            flex: 1,
            padding: '8px 12px',
            border: '1px solid #d1d5db',
            borderRadius: 6,
            fontSize: 14,
          }}
        />
        <button
          onClick={fetchEscrow}
          disabled={loading || !escrowId}
          style={{
            padding: '8px 16px',
            background: '#3b82f6',
            color: 'white',
            border: 'none',
            borderRadius: 6,
            cursor: loading ? 'wait' : 'pointer',
          }}
        >
          {loading ? 'Loading...' : 'View'}
        </button>
      </div>

      {error && (
        <div style={{ padding: 12, background: '#fef2f2', color: '#dc2626', borderRadius: 6, marginBottom: 16 }}>
          {error}
        </div>
      )}

      {escrow && (
        <div style={{ marginBottom: 24 }}>
          <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 16, marginBottom: 16 }}>
            <div style={{ padding: 16, background: '#f9fafb', borderRadius: 8 }}>
              <div style={{ fontSize: 12, color: '#6b7280', marginBottom: 4 }}>Status</div>
              <div style={{ fontSize: 18, fontWeight: 'bold', color: statusColor(escrow.status) }}>
                {escrow.status}
              </div>
            </div>
            <div style={{ padding: 16, background: '#f9fafb', borderRadius: 8 }}>
              <div style={{ fontSize: 12, color: '#6b7280', marginBottom: 4 }}>Escrow ID</div>
              <div style={{ fontSize: 18, fontWeight: 'bold' }}>#{escrow.escrowId}</div>
            </div>
          </div>

          <div style={{ padding: 16, background: '#f9fafb', borderRadius: 8, marginBottom: 16 }}>
            <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 16 }}>
              <div>
                <div style={{ fontSize: 12, color: '#6b7280', marginBottom: 4 }}>Funder</div>
                <div style={{ fontSize: 14, wordBreak: 'break-all' }}>{escrow.funder}</div>
              </div>
              <div>
                <div style={{ fontSize: 12, color: '#6b7280', marginBottom: 4 }}>Receiver</div>
                <div style={{ fontSize: 14, wordBreak: 'break-all' }}>{escrow.receiver}</div>
              </div>
              <div>
                <div style={{ fontSize: 12, color: '#6b7280', marginBottom: 4 }}>Deposited</div>
                <div style={{ fontSize: 14, fontWeight: 'bold' }}>{escrow.totalDeposited}</div>
              </div>
              <div>
                <div style={{ fontSize: 12, color: '#6b7280', marginBottom: 4 }}>Released</div>
                <div style={{ fontSize: 14, fontWeight: 'bold' }}>{escrow.totalReleased}</div>
              </div>
            </div>
          </div>

          <h2 style={{ fontSize: 18, fontWeight: 'bold', marginBottom: 12 }}>Milestones</h2>
          <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
            {escrow.milestones?.map((m, i) => (
              <div
                key={i}
                style={{
                  padding: 12,
                  background: '#f9fafb',
                  borderRadius: 8,
                  borderLeft: `4px solid ${statusColor(m.status)}`,
                }}
              >
                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                  <div>
                    <div style={{ fontWeight: 'bold' }}>Milestone {i + 1}</div>
                    <div style={{ fontSize: 14, color: '#6b7280' }}>{m.description}</div>
                  </div>
                  <div style={{ textAlign: 'right' }}>
                    <div style={{ fontSize: 12, color: '#6b7280' }}>Status</div>
                    <div style={{ color: statusColor(m.status), fontWeight: 'bold' }}>{m.status}</div>
                  </div>
                </div>
              </div>
            ))}
          </div>
        </div>
      )}

      {events.length > 0 && (
        <div>
          <h2 style={{ fontSize: 18, fontWeight: 'bold', marginBottom: 12 }}>Recent Events</h2>
          <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
            {events.map((e) => (
              <div
                key={e.id}
                style={{
                  padding: 12,
                  background: '#f9fafb',
                  borderRadius: 8,
                  fontSize: 14,
                }}
              >
                <div style={{ display: 'flex', justifyContent: 'space-between' }}>
                  <span style={{ fontWeight: 'bold' }}>{e.event_type}</span>
                  <span style={{ color: '#6b7280' }}>Ledger {e.ledger}</span>
                </div>
                <div style={{ color: '#6b7280', fontSize: 12, marginTop: 4 }}>
                  TX: {e.tx_hash.slice(0, 16)}...
                </div>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
