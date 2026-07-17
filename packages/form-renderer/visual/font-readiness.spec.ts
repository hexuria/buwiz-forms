import { expect, test, type Page } from "@playwright/test";

type RendererHostMessage = {
  type?: string;
  message?: string;
};

type RendererTestWindow = Window & {
  __EBIR_TEST_HOST_MESSAGES__?: string[];
  prepareEbirFormForNativePrint?: (nonce: number) => void;
};

async function installHostMessageRecorder(page: Page) {
  await page.addInitScript(() => {
    const messages: string[] = [];
    Object.defineProperty(window, "__EBIR_TEST_HOST_MESSAGES__", {
      configurable: false,
      value: messages
    });
    Object.defineProperty(window, "ipc", {
      configurable: false,
      value: {
        postMessage(message: string) {
          messages.push(message);
        }
      }
    });
  });
}

test("the shipped Arimo faces permit renderer and native-print readiness", async ({
  page
}) => {
  await installHostMessageRecorder(page);
  await page.goto("/");
  await page.waitForFunction(() => {
    const encoded =
      (window as RendererTestWindow).__EBIR_TEST_HOST_MESSAGES__ ?? [];
    return encoded
      .map((message) => JSON.parse(message) as RendererHostMessage)
      .some((message) => message.type === "renderer_ready");
  });

  await page.evaluate(() => {
    const prepare = (window as RendererTestWindow).prepareEbirFormForNativePrint;
    if (!prepare) throw new Error("Native print preflight is unavailable");
    prepare(70);
  });
  await page.waitForFunction(() => {
    const encoded =
      (window as RendererTestWindow).__EBIR_TEST_HOST_MESSAGES__ ?? [];
    return encoded
      .map(
        (message) =>
          JSON.parse(message) as RendererHostMessage & { nonce?: number }
      )
      .some((message) => message.type === "print_ready" && message.nonce === 70);
  });

  const messages = await page.evaluate(() => {
    const encoded =
      (window as RendererTestWindow).__EBIR_TEST_HOST_MESSAGES__ ?? [];
    return encoded.map(
      (message) => JSON.parse(message) as RendererHostMessage
    );
  });
  expect(messages.some((message) => message.type === "renderer_error")).toBe(
    false
  );
});

test("a failed bundled-font request emits only renderer errors", async ({
  page
}) => {
  let blockedFontRequests = 0;
  await page.route(/arimo-.*\.woff2(?:\?.*)?$/u, async (route) => {
    blockedFontRequests += 1;
    await route.abort("failed");
  });
  await installHostMessageRecorder(page);

  await page.goto("/");
  await page.waitForFunction(() => {
    const encoded =
      (window as RendererTestWindow).__EBIR_TEST_HOST_MESSAGES__ ?? [];
    const messages = encoded.map(
      (message) => JSON.parse(message) as RendererHostMessage
    );
    return messages.some(
      (message) =>
        message.type === "renderer_error" &&
        message.message?.includes("Required bundled printable font face")
    );
  });

  const errorsBeforePrint = await page.evaluate(() => {
    const encoded =
      (window as RendererTestWindow).__EBIR_TEST_HOST_MESSAGES__ ?? [];
    return encoded
      .map((message) => JSON.parse(message) as RendererHostMessage)
      .filter((message) => message.type === "renderer_error").length;
  });
  await page.evaluate(() => {
    const prepare = (window as RendererTestWindow).prepareEbirFormForNativePrint;
    if (!prepare) throw new Error("Native print preflight is unavailable");
    prepare(71);
  });
  await page.waitForFunction(
    (previousCount) => {
      const encoded =
        (window as RendererTestWindow).__EBIR_TEST_HOST_MESSAGES__ ?? [];
      const errors = encoded
        .map((message) => JSON.parse(message) as RendererHostMessage)
        .filter((message) => message.type === "renderer_error").length;
      return errors > previousCount;
    },
    errorsBeforePrint
  );

  // Give any stale geometry timer an opportunity to fire. A font failure must
  // still leave both normal and native-print readiness fail-closed.
  await page.waitForTimeout(150);
  const messages = await page.evaluate(() => {
    const encoded =
      (window as RendererTestWindow).__EBIR_TEST_HOST_MESSAGES__ ?? [];
    return encoded.map(
      (message) => JSON.parse(message) as RendererHostMessage
    );
  });
  expect(blockedFontRequests).toBeGreaterThan(0);
  expect(messages.some((message) => message.type === "renderer_error")).toBe(
    true
  );
  expect(messages.some((message) => message.type === "renderer_ready")).toBe(
    false
  );
  expect(messages.some((message) => message.type === "print_ready")).toBe(
    false
  );
});
