from playwright.sync_api import sync_playwright

def run(playwright):
    browser = playwright.chromium.launch()
    page = browser.new_page()
    # The dev server runs on port 1420 by default
    page.goto("http://localhost:1420")

    # Send a message to trigger the dynamic component
    page.get_by_placeholder("Type your message here...").fill("Hello")
    page.get_by_role("button", name="Send").click()

    # Wait for the dynamic component to be rendered
    page.wait_for_selector("text=This is a dynamic component from the backend!")

    # Take a screenshot
    page.screenshot(path="jules-scratch/verification/verification.png")

    browser.close()

with sync_playwright() as playwright:
    run(playwright)
