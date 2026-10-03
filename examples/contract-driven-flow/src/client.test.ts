import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { OrderClient, computeOrderTax, type Order } from "./client.js";

describe("OrderClient contract tests", () => {
  it("computes order tax based on total_cents", () => {
    const mockOrder: Order = {
      id: "550e8400-e29b-41d4-a716-446655440000",
      sku: "RUST-CORE-01",
      quantity: 2,
      total_cents: 4999,
      status: "pending",
      created_at: new Date().toISOString(),
    };

    const tax = computeOrderTax(mockOrder, 0.19);
    assert.equal(tax, 950);
  });

  it("handles order creation with contract compliance", async () => {
    const mockOrder: Order = {
      id: "550e8400-e29b-41d4-a716-446655440001",
      sku: "ONAJI-CLI-PRO",
      quantity: 1,
      total_cents: 9900,
      status: "processing",
      created_at: new Date().toISOString(),
    };

    const mockFetch = async () =>
      new Response(JSON.stringify(mockOrder), {
        status: 201,
        headers: { "Content-Type": "application/json" },
      });

    const client = new OrderClient({
      baseUrl: "https://api.example.com",
      fetchFn: mockFetch as unknown as typeof fetch,
    });

    const created = await client.createOrder({
      sku: "ONAJI-CLI-PRO",
      quantity: 1,
      total_cents: 9900,
    });

    assert.equal(created.id, mockOrder.id);
    assert.equal(created.status, "processing");
  });
});
