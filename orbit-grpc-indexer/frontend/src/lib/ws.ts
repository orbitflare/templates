"use client";

import { useEffect, useRef, useState, useCallback } from "react";

export interface LiveTransaction {
  signature: string;
  slot: number;
  source: string;
  success: boolean;
  has_cpi_data: boolean;
}

export function useLiveTransactions(maxItems: number = 30) {
  const [transactions, setTransactions] = useState<LiveTransaction[]>([]);
  const [connected, setConnected] = useState(false);
  const wsRef = useRef<WebSocket | null>(null);
  const reconnectTimeout = useRef<ReturnType<typeof setTimeout> | null>(null);
  const bufferRef = useRef<LiveTransaction[]>([]);
  const flushInterval = useRef<ReturnType<typeof setInterval> | null>(null);

  const connect = useCallback(() => {
    if (wsRef.current?.readyState === WebSocket.OPEN) return;

    const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
    const wsUrl = `${protocol}//${window.location.hostname}:3000/ws/transactions`;
    const ws = new WebSocket(wsUrl);

    ws.onopen = () => {
      setConnected(true);
    };

    ws.onmessage = (event) => {
      try {
        const tx: LiveTransaction = JSON.parse(event.data);
        if (tx.success) {
          bufferRef.current.push(tx);
        }
      } catch {
        // skip
      }
    };

    ws.onclose = () => {
      setConnected(false);
      reconnectTimeout.current = setTimeout(connect, 3000);
    };

    ws.onerror = () => {
      ws.close();
    };

    wsRef.current = ws;
  }, []);

  useEffect(() => {
    connect();

    flushInterval.current = setInterval(() => {
      if (bufferRef.current.length > 0) {
        const batch = bufferRef.current.splice(0, 3);
        bufferRef.current = [];
        setTransactions((prev) => [...batch.reverse(), ...prev].slice(0, maxItems));
      }
    }, 2000);

    return () => {
      if (reconnectTimeout.current) clearTimeout(reconnectTimeout.current);
      if (flushInterval.current) clearInterval(flushInterval.current);
      wsRef.current?.close();
    };
  }, [connect, maxItems]);

  return { transactions, connected };
}
