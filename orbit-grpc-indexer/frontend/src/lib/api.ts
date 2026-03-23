const BASE = "";

export interface HealthData {
  healthy: boolean;
  jetstream_connected: boolean;
  yellowstone_connected: boolean;
  database_connected: boolean;
  last_indexed_slot: number;
  transactions_indexed: number;
}

export interface Transaction {
  signature: string;
  slot: number;
  source: string;
  success: boolean;
  num_instructions: number | null;
  has_cpi_data: boolean;
  indexed_at: string;
  fee: number | null;
  error: unknown | null;
  account_keys: string[];
  accounts: string[];
  log_messages: string[];
  block_time: string | null;
  raw: unknown | null;
  enriched_at: string | null;
}

export interface InnerInstruction {
  id: number;
  signature: string;
  instruction_idx: number;
  depth: number;
  program_id: string;
  accounts: string[];
  data: string | null;
  indexed_at: string;
}

export interface TransactionDetail {
  transaction: Transaction;
  inner_instructions: InnerInstruction[];
}

export interface CursorPagination {
  next_cursor: string | null;
  limit: number;
  has_more: boolean;
}

export interface TransactionsResponse {
  data: Transaction[];
  pagination: CursorPagination;
}

export interface TransactionDetailResponse {
  data: TransactionDetail;
}

export interface AccountTransactionsResponse {
  data: Transaction[];
  pagination: CursorPagination;
}

export async function fetchHealth(): Promise<HealthData> {
  const res = await fetch(`${BASE}/health`);
  if (!res.ok) throw new Error("Failed to fetch health");
  return res.json();
}

export interface TransactionFilters {
  limit?: number;
  cursor?: string;
  program_id?: string;
  account?: string;
  success?: boolean;
  slot_min?: number;
  slot_max?: number;
  source?: string;
}

export async function fetchTransactions(
  filters: TransactionFilters = {}
): Promise<TransactionsResponse> {
  const params = new URLSearchParams();
  params.set("pagination", "cursor");
  if (filters.limit) params.set("limit", String(filters.limit));
  if (filters.cursor) params.set("cursor", filters.cursor);
  if (filters.program_id) params.set("program_id", filters.program_id);
  if (filters.source) params.set("source", filters.source);
  if (filters.account) params.set("account", filters.account);
  if (filters.success !== undefined)
    params.set("success", String(filters.success));
  if (filters.slot_min) params.set("slot_min", String(filters.slot_min));
  if (filters.slot_max) params.set("slot_max", String(filters.slot_max));
  const res = await fetch(`${BASE}/api/v1/transactions?${params.toString()}`);
  if (!res.ok) throw new Error("Failed to fetch transactions");
  return res.json();
}

export async function fetchTransaction(
  signature: string
): Promise<TransactionDetailResponse> {
  const res = await fetch(`${BASE}/api/v1/transactions/${signature}`);
  if (!res.ok) throw new Error("Failed to fetch transaction");
  return res.json();
}

export async function fetchAccountTransactions(
  address: string,
  filters: { limit?: number; cursor?: string } = {}
): Promise<AccountTransactionsResponse> {
  const params = new URLSearchParams();
  if (filters.limit) params.set("limit", String(filters.limit));
  if (filters.cursor) params.set("cursor", filters.cursor);
  const res = await fetch(
    `${BASE}/api/v1/accounts/${address}/transactions?${params.toString()}`
  );
  if (!res.ok) throw new Error("Failed to fetch account transactions");
  return res.json();
}
