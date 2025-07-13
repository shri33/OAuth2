#!/bin/bash

# Production deployment script for Shopify OAuth Rust application

set -e

echo "🚀 Starting production deployment..."

# Load environment variables
if [ -f .env.production ]; then
    export $(cat .env.production | xargs)
    echo "✅ Loaded production environment variables"
else
    echo "❌ .env.production file not found"
    exit 1
fi

# Validate required environment variables
required_vars=("SHOP" "API_KEY" "API_SECRET" "REDIRECT_URI" "DATABASE_URL" "ENCRYPTION_KEY")
for var in "${required_vars[@]}"; do
    if [ -z "${!var}" ]; then
        echo "❌ Required environment variable $var is not set"
        exit 1
    fi
done
echo "✅ All required environment variables are set"

# Create backup of current deployment (if exists)
if [ -d "backup" ]; then
    rm -rf backup.old
    mv backup backup.old
fi
mkdir -p backup

# Backup database
echo "🔄 Creating database backup..."
pg_dump $DATABASE_URL > backup/database_$(date +%Y%m%d_%H%M%S).sql
echo "✅ Database backup created"

# Pull latest code
echo "🔄 Pulling latest code..."
git pull origin main
echo "✅ Code updated"

# Build and deploy with Docker
echo "🔄 Building application..."
docker-compose -f docker-compose.prod.yml build --no-cache
echo "✅ Application built"

# Run database migrations
echo "🔄 Running database migrations..."
docker-compose -f docker-compose.prod.yml run --rm shopify-oauth-migrate
echo "✅ Database migrations completed"

# Start services
echo "🔄 Starting services..."
docker-compose -f docker-compose.prod.yml up -d
echo "✅ Services started"

# Health check
echo "🔄 Performing health check..."
sleep 10
for i in {1..30}; do
    if curl -f http://localhost/health 2>/dev/null; then
        echo "✅ Application is healthy"
        break
    fi
    if [ $i -eq 30 ]; then
        echo "❌ Health check failed"
        echo "🔄 Rolling back..."
        docker-compose -f docker-compose.prod.yml down
        # Restore from backup logic here
        exit 1
    fi
    sleep 2
done

# SSL certificate renewal (if using Let's Encrypt)
if [ -f /usr/bin/certbot ]; then
    echo "🔄 Renewing SSL certificates..."
    certbot renew --quiet
    echo "✅ SSL certificates renewed"
fi

echo "🎉 Deployment completed successfully!"
echo "🌐 Application is running at: https://$DOMAIN"
