import { Connection, clusterApiUrl, PublicKey } from "@solana/web3.js";
import { parsePriceData } from "@pythnetwork/client";
import dotenv from "dotenv";

dotenv.config();

const BTC_PRICE_FEED = new PublicKey("3E2hGM7WQVWyBKFYCY9ABvxP8T8SgfAisgfRxwq2EUCi");
const HTTP_ENDPOINT = process.env.SOLANA_RPC_HTTP ?? clusterApiUrl("mainnet-beta");
const WS_ENDPOINT = process.env.SOLANA_RPC_WS;

async function main() {
  console.log("Connecting to HTTP", HTTP_ENDPOINT);
  if (WS_ENDPOINT) {
    console.log("Using WebSocket", WS_ENDPOINT);
  }

  const connection = new Connection(HTTP_ENDPOINT, {
    commitment: "confirmed",
    wsEndpoint: WS_ENDPOINT,
  });

  const initialInfo = await connection.getAccountInfo(BTC_PRICE_FEED);
  if (initialInfo) {
    try {
      const initialData = parsePriceData(initialInfo.data);
      console.log(
        `Initial BTC/USD: ${initialData.price} ± ${initialData.confidence} (status: ${initialData.status ?? "unknown"})`
      );
    } catch (err) {
      console.error("Failed to decode initial price feed", err);
    }
  } else {
    console.warn("No initial account data fetched for BTC price feed");
  }

  const subscriptionId = connection.onAccountChange(
    BTC_PRICE_FEED,
    (accountInfo) => {
      try {
        const priceData = parsePriceData(accountInfo.data);

        const price = priceData.price;
        const confidence = priceData.confidence;
        const status = priceData.status ?? "unknown";

        if (price !== undefined) {
          console.log(
            `BTC/USD: ${price} ± ${confidence} (status: ${status}) @ ${new Date().toISOString()}`
          );
        } else {
          console.warn("Price unavailable in latest update");
        }
      } catch (err) {
        console.error("Failed to decode price feed", err);
      }
    },
    "confirmed"
  );

  process.on("SIGINT", async () => {
    console.log("\nUnsubscribing...");
    await connection.removeAccountChangeListener(subscriptionId);
    process.exit(0);
  });
}

main().catch((err) => {
  console.error("Fatal error streaming price", err);
  process.exit(1);
});
