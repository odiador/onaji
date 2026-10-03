import type { components, paths } from "./generated/api.d.ts";

export type Order = components["schemas"]["Order"];
export type CreateOrderRequest = components["schemas"]["CreateOrderRequest"];
export type OrderStatus = Order["status"];

export interface ApiClientConfig {
  baseUrl: string;
  fetchFn?: typeof fetch;
}

export class OrderClient {
  private readonly baseUrl: string;
  private readonly fetchFn: typeof fetch;

  constructor(config: ApiClientConfig) {
    this.baseUrl = config.baseUrl.replace(/\/$/, "");
    this.fetchFn = config.fetchFn ?? fetch;
  }

  async listOrders(): Promise<Order[]> {
    const res = await this.fetchFn(`${this.baseUrl}/orders`);
    if (!res.ok) {
      throw new Error(`Failed to list orders: HTTP ${res.status}`);
    }
    return res.json() as Promise<paths["/orders"]["get"]["responses"]["200"]["content"]["application/json"]>;
  }

  async createOrder(req: CreateOrderRequest): Promise<Order> {
    const res = await this.fetchFn(`${this.baseUrl}/orders`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(req),
    });
    if (!res.ok) {
      throw new Error(`Failed to create order: HTTP ${res.status}`);
    }
    return res.json() as Promise<paths["/orders"]["post"]["responses"]["201"]["content"]["application/json"]>;
  }

  async getOrder(id: string): Promise<Order> {
    const res = await this.fetchFn(`${this.baseUrl}/orders/${encodeURIComponent(id)}`);
    if (!res.ok) {
      throw new Error(`Order ${id} not found: HTTP ${res.status}`);
    }
    return res.json() as Promise<paths["/orders/{id}"]["get"]["responses"]["200"]["content"]["application/json"]>;
  }
}

export function computeOrderTax(order: Order, taxRate: number): number {
  return Math.round(order.total_cents * taxRate);
}
