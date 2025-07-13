#!/usr/bin/env bash

# Simple test script to verify PostgreSQL connection
echo "Testing PostgreSQL connection..."

# Try different connection methods
echo "Method 1: Default postgres database"
"C:/Program Files/PostgreSQL/13/bin/psql.exe" -U postgres -d postgres -c "SELECT version();" 2>&1

echo ""
echo "Method 2: List databases"
"C:/Program Files/PostgreSQL/13/bin/psql.exe" -U postgres -l 2>&1

echo ""
echo "Method 3: Create database"
"C:/Program Files/PostgreSQL/13/bin/createdb.exe" -U postgres shopify_oauth 2>&1

echo ""
echo "Method 4: Test connection to new database"
"C:/Program Files/PostgreSQL/13/bin/psql.exe" -U postgres -d shopify_oauth -c "SELECT 1;" 2>&1
