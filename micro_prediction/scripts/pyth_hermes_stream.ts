import dotenv from "dotenv";

dotenv.config();

const HERMES_REST_ENDPOINT =
  process.env.PYTH_HERMES_REST_ENDPOINT ??
  "https://hermes.pyth.network/api/latest_price_feeds";
const BTC_FEED_ID =
  process.env.PYTH_BTC_FEED_ID ??
  "0xe62df6c8b4a85fe1a67db44dc12de5db330f7ac66b72dc658afedf0f4a415b43";
const POLL_INTERVAL_MS = Number(process.env.PYTH_POLL_INTERVAL_MS ?? 1000);

async function fetchLatestPrice() {
  const url = new URL(HERMES_REST_ENDPOINT);
  url.searchParams.append("ids[]", BTC_FEED_ID);

  try {
    const response = await fetch(url.toString());
    if (!response.ok) {
      throw new Error(`HTTP ${response.status} ${response.statusText}`);
    }

    const data = (await response.json()) as Array<{
      id: string;
      price?: {
        price: string;
        conf: string;
        expo: number;
        publish_time: number;
      };
      ema_price?: {
        price: string;
        conf: string;
        expo: number;
        publish_time: number;
      };
    }>;

    if (!data.length || !data[0].price) {
      console.warn("No price data returned from Hermes");
      return;
    }

    const { price, conf, expo, publish_time } = data[0].price;
    const scaledPrice = Number(price) * 10 ** expo;
    const scaledConf = Number(conf) * 10 ** expo;
    const timestamp = new Date(publish_time * 1000).toISOString();

    console.log(
      `BTC/USD: ${scaledPrice.toFixed(2)} ±${scaledConf.toFixed(
        2
      )} at ${timestamp}`
    );
  } catch (err) {
    console.error("Failed to fetch price from Hermes", err);
  }
}

async function main() {
  console.log("Streaming BTC price via Pyth Hermes REST");
  console.log(`Feed ID: ${BTC_FEED_ID}`);
  console.log(`Polling every ${POLL_INTERVAL_MS}ms`);

  await fetchLatestPrice();
  setInterval(fetchLatestPrice, POLL_INTERVAL_MS);
}

main().catch((err) => {
  console.error("Fatal error in Hermes streamer", err);
  process.exit(1);
});
