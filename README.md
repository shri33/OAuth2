🛍️ Shopify OAuth Rust Application

    

A production-ready Rust application for Shopify OAuth 2.0. Secure, performant, and focused on core Shopify integration.


---

📋 Table of Contents

1. Navigation


2. What It Does


3. Quick Start


4. Architecture Overview


5. API & Endpoints


6. Development Guide


7. Deployment


8. Configuration


9. Troubleshooting


10. Performance & Scaling


11. Contributing


12. License & Acknowledgments




---

🔗 Navigation

Code

Issues

Pull Requests

Actions

Security

Wiki



---

🌟 What It Does

🔐 OAuth 2.0 Flow: Merchant authorization, token exchange, CSRF protection.

🛡️ Security: AES-256-GCM token encryption, HMAC validation, rate limiting.

📦 Shopify API: Orders, Products, Customers, Inventory, Abandoned Checkouts.

⚡ Webhooks: Real-time event handling with HMAC verification.

🚀 Production Ready: Dockerized, migrations, comprehensive tests.



---

🎯 Quick Start

1. Clone

git clone https://github.com/shri33/OAuth2.git
cd OAuth2


2. Set up

cp .env.example .env
# Fill in your Shopify and DB credentials


3. Build & Run

cargo build
cargo run


4. Test OAuth

Visit http://localhost:3000 → Connect to Shopify → Complete authorization.





---

🏗️ Architecture Overview

OAuth Flow

sequenceDiagram
  participant Merchant
  participant App
  participant Shopify
  participant DB
  Merchant->>App: GET /auth
  App->>DB: store CSRF state
  App->>Merchant: redirect to Shopify
  Merchant->>Shopify: authorize
  Shopify->>App: callback w/ code
  App->>DB: validate state
  App->>Shopify: exchange code
  Shopify->>App: return token
  App->>DB: store encrypted token
  App->>Merchant: success page

API Request Flow

sequenceDiagram
  participant Client
  participant App
  participant DB
  participant Shopify
  Client->>App: GET /orders
  App->>DB: fetch token
  DB-->>App: decrypted token
  App->>Shopify: request orders
  Shopify-->>App: orders data
  App-->>Client: JSON response

Project Structure

shopify-oauth-rust/
├── src/                   # Application source (3,272 LOC)
├── migrations/            # DB schema
├── Dockerfile*            # Docker configs (standard, scratch, ultra-secure)
├── docker-compose.yml     # Full-stack deployment
├── .env.example           # Environment template
└── README.md


---

📈 API & Endpoints

OAuth

Path	Method	Description

/auth	GET	Start OAuth
/callback	GET	Handle callback & token


Shopify APIs

Path	Method	Description

/api/orders	GET	Fetch orders
/api/products	GET	Fetch products
/api/customers	GET	Fetch customers
/api/inventory	GET	Check inventory levels
/api/abandoned-checkouts	GET	Abandoned checkouts


Webhooks

Path	Method	Description

/webhooks/orders/created	POST	New order events
/webhooks/products/created	POST	New product events
/webhooks/customers/created	POST	New customer registrations



---

🛠️ Development Guide

# Run tests
cargo test
# Lint and format
cargo clippy && cargo fmt
# Watch & reload
tools: cargo watch -x run

Add endpoints in src/shopify_api.rs & routes in src/main.rs. Write tests in src/tests.rs.


---

🚀 Deployment

Docker Compose

cp .env.example .env.production
# configure .env.production
docker-compose up -d

Ultra-Secure Image

docker build -f Dockerfile.pure-scratch -t shopify-oauth-rust .
docker run -p 3000:3000 --env-file .env production


---

⚙️ Configuration

Fill .env:

SHOP=your-shop.myshopify.com
API_KEY=...
API_SECRET=...
REDIRECT_URI=http://localhost:3000/callback
DATABASE_URL=postgresql://user:pass@localhost:5432/shopify_oauth
ENCRYPTION_KEY=<32-byte hex>
JWT_SECRET=<base64>


---

🔍 Troubleshooting

Invalid redirect URI: Match REDIRECT_URI in Partner Dashboard.

CSRF failed: Check DB connectivity for state storage.

429 Rate Limit: Adjust retry logic or rate limits.



---

📊 Performance & Scaling

Async I/O via Tokio

Connection Pooling

Horizontal Scaling: Stateless design

Caching: Redis support



---

🤝 Contributing

1. Fork & branch


2. Code & tests


3. Format & lint


4. PR for review




---

📄 License & Acknowledgments

Licensed under MIT. See LICENSE.

Built with ❤️ by Shri Srivastava 

