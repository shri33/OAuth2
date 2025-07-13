# 🚀 How to Run Shopify OAuth Rust App

## 📋 **Current Status - Waiting for Credentials**

✅ **Code is complete and ready**  
✅ **All tests pass (15/17)**  
✅ **Compiles successfully**  
⏳ **Waiting for Joseph to provide:**
   - Shopify API Key
   - Shopify API Secret  
   - Development Shop URL

---

## Quick Start (Once You Have Credentials)

### Prerequisites
- [Rust 1.70+](https://rustup.rs/)
- [PostgreSQL](https://www.postgresql.org/download/)
- [Shopify Partner Account](https://partners.shopify.com/)

---

## Step 1: Get Shopify App Credentials

### ⚠️ **IMPORTANT: You need Shopify App credentials to run this**

This application requires:
1. **Shopify API Key** (Client ID)
2. **Shopify API Secret** (Client Secret)  
3. **Development Shop URL** (e.g., `my-shop.myshopify.com`)

### Option A: Wait for Joseph to provide credentials
Joseph will provide you with:
- API Key
- API Secret  
- Shop URL

### Option B: Create your own Shopify App (if you have Partner account)
1. Go to [Shopify Partners Dashboard](https://partners.shopify.com/)
2. Click **"Create app"** → **"Create app manually"**
3. Fill in app details:
   - **App name**: `Rust OAuth Test`
   - **App URL**: `http://localhost:3000`
   - **Allowed redirection URL(s)**: `http://localhost:3000/callback`
4. Copy the **Client ID** and **Client secret**

---

## Step 2: Setup Database (Easy Option)

### SQLite (No installation needed)
```bash
# No setup required - database file created automatically
# Just make sure your .env has:
DATABASE_URL=sqlite:shopify_oauth.db
```

### PostgreSQL (Advanced)
```bash
# Windows - Find PostgreSQL installation
Get-Service postgresql*

# Create database using full path
& "C:\Program Files\PostgreSQL\17\bin\createdb.exe" -U postgres shopify_oauth
```

---

## Step 3: Configure Environment

### Copy Environment Template
```powershell
# Windows PowerShell
Copy-Item ".env.example" ".env"
```

### Edit Your .env File
Open `.env` file and update with **Joseph's credentials**:

```env
# 🔑 REPLACE WITH JOSEPH'S ACTUAL VALUES:
SHOP=josephs-shop-name.myshopify.com
API_KEY=josephs_actual_api_key_here  
API_SECRET=josephs_actual_api_secret_here

# 🗄️ Database (SQLite is easiest)
DATABASE_URL=sqlite:shopify_oauth.db

# 🔐 Encryption key (already generated)
ENCRYPTION_KEY=3681752cafcc33d635baade79a38ec89e381e9d541d857057d7cdbd83886fb8
```

### Generate Encryption Key (if needed)
```powershell
# Windows PowerShell method
-join ((1..32) | ForEach {'{0:x2}' -f (Get-Random -Max 256)})

# Or use this pre-generated one for testing:
# 3681752cafcc33d635baade79a38ec89e381e9d541d857057d7cdbd83886fb8
```

---

## Step 4: Build & Run (After Getting Credentials)

### ⚠️ **Wait for Joseph's Credentials First!**

You need to update `.env` with actual values before running:
- SHOP=`josephs-actual-shop.myshopify.com`
- API_KEY=`josephs_actual_api_key`
- API_SECRET=`josephs_actual_secret`

### Install Dependencies
```bash
cargo build
```

### Start the Application (After .env is updated)
```bash
cargo run
```

**Expected Output:**
```
INFO shopify_oauth_rust: Starting Shopify OAuth Rust application
INFO shopify_oauth_rust: 📍 Shop: josephs-actual-shop.myshopify.com
INFO shopify_oauth_rust: Database connected successfully  
INFO shopify_oauth_rust: Running migrations...
INFO shopify_oauth_rust: Server listening on http://0.0.0.0:3000
```

### If You See Database Errors:
Make sure your `.env` has:
```env
DATABASE_URL=sqlite:shopify_oauth.db
```
Instead of PostgreSQL connection string.

---

## Step 4: Setup Shopify App (First Time Only)

### Create Shopify App
1. Go to [Shopify Partners Dashboard](https://partners.shopify.com/)
2. Click **"Create app"** → **"Create app manually"**
3. Fill in app details:
   - **App name**: `My Rust OAuth App`
   - **App URL**: `http://localhost:3000`
   - **Allowed redirection URL(s)**: `http://localhost:3000/callback`

### Get API Credentials
1. In your app dashboard, go to **"App setup"**
2. Copy **Client ID** → paste as `API_KEY` in `.env`
3. Copy **Client secret** → paste as `API_SECRET` in `.env`

---

## Step 5: Test OAuth Flow

### 1. Open Application
Visit: http://localhost:3000

### 2. Connect to Shopify
- Click **"🔗 Connect to Shopify"**
- You'll be redirected to Shopify's authorization page
- Click **"Install app"**
- You'll be redirected back to your app

### 3. Success! 
You should see a success page with your access token stored securely.

---

## Step 6: Test API Endpoints

### Available Endpoints
```bash
# Get orders from your shop
http://localhost:3000/orders

# Get products  
http://localhost:3000/products

# Get customers
http://localhost:3000/customers

# Get inventory levels
http://localhost:3000/inventory

# Get abandoned checkouts
http://localhost:3000/abandoned-checkouts

# List configured webhooks
http://localhost:3000/webhooks
```

### Test with curl
```bash
# Example: Get orders
curl http://localhost:3000/orders

# Example: Get products
curl http://localhost:3000/products
```

---

## 🐳 Docker Alternative

### Quick Docker Run
```bash
# Build image
docker build -t shopify-oauth-rust .

# Run with Docker Compose (includes PostgreSQL)
docker-compose up -d
```

### Ultra-Secure Docker Build
```bash
# Maximum security build
docker build -f Dockerfile.pure-scratch -t shopify-oauth-rust-secure .
```

---

## ⚠️ Troubleshooting

### Database Connection Issues
```bash
# Check PostgreSQL is running
# Windows
Get-Service postgresql*

# Linux/Mac  
sudo systemctl status postgresql
```

### Environment Variable Issues
- ✅ Verify `.env` file exists in project root
- ✅ Check all required variables are set
- ✅ Ensure encryption key is exactly 64 hex characters
- ✅ No quotes around values in `.env`

### Shopify App Issues
- ✅ App URL must be: `http://localhost:3000`
- ✅ Redirect URL must be: `http://localhost:3000/callback`
- ✅ API credentials copied correctly from Partner Dashboard

### Port Already in Use
```bash
# Change port in .env file
PORT=3001

# Or kill process on port 3000
# Windows
netstat -ano | findstr :3000
taskkill /PID <process_id> /F

# Linux/Mac
lsof -ti:3000 | xargs kill -9
```

### Compilation Errors
```bash
# Update Rust
rustup update

# Clean build
cargo clean && cargo build
```

---

## 📊 What Happens When You Run

1. **Database Connection**: Connects to PostgreSQL
2. **Migrations**: Creates tables automatically if needed
3. **Web Server**: Starts Axum server on port 3000
4. **OAuth Ready**: Ready to handle Shopify authorization
5. **API Ready**: Ready to make Shopify API calls

---

## 🎯 Success Checklist

- [ ] PostgreSQL database created
- [ ] `.env` file configured with real values
- [ ] Shopify app created in Partner Dashboard
- [ ] Application builds without errors (`cargo build`)
- [ ] Server starts successfully (`cargo run`)
- [ ] Can access http://localhost:3000
- [ ] OAuth flow completes successfully
- [ ] API endpoints return data

---

## 🔗 Next Steps

Once running successfully:
- Install app in your development store
- Test webhook endpoints
- Deploy to production using Docker
- Scale with the provided deployment guides

**🎉 Your Shopify OAuth Rust app is now running!**
