#!/usr/bin/env python3
"""
Generate a Postman collection from the OpenAPI specification.
"""
import json
import os
import sys

def generate_postman_collection(openapi_file, output_file):
    # Read the OpenAPI specification
    with open(openapi_file, 'r') as f:
        spec = json.load(f)
    
    # Create the Postman collection structure
    collection = {
        "info": {
            "name": "Degen API",
            "description": "API for managing cryptocurrency wallets",
            "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json"
        },
        "item": []
    }
    
    # Add each endpoint to the collection
    for path, methods in spec.get('paths', {}).items():
        for method, details in methods.items():
            if method.lower() not in ['get', 'post', 'put', 'delete', 'patch']:
                continue
                
            # Create a request item
            request = {
                "name": details.get('summary', f"{method.upper()} {path}"),
                "request": {
                    "method": method.upper(),
                    "header": [
                        {
                            "key": "Content-Type",
                            "value": "application/json"
                        }
                    ],
                    "url": {
                        "raw": f"{{{{base_url}}}}{path}",
                        "host": ["{{base_url}}"],
                        "path": path.strip('/').split('/')
                    }
                },
                "response": []
            }
            
            # Add request body if it exists
            if 'requestBody' in details:
                request["request"]["body"] = {
                    "mode": "raw",
                    "raw": json.dumps({
                        "address": "0x742d35Cc6634C0532925a3b844Bc454e4438f44e"
                    }),
                    "options": {
                        "raw": {
                            "language": "json"
                        }
                    }
                }
            
            # Add the request to the collection
            collection["item"].append({
                "name": f"{method.upper()} {path}",
                "item": [request]
            })
    
    # Add an environment template
    environment = {
        "name": "Degen API Environment",
        "values": [
            {
                "key": "base_url",
                "value": "http://localhost:3000",
                "enabled": True
            }
        ]
    }
    
    # Create the output directory if it doesn't exist
    os.makedirs(os.path.dirname(output_file), exist_ok=True)
    
    # Write the collection to a file
    with open(output_file, 'w') as f:
        json.dump(collection, f, indent=2)
    
    # Also write the environment to a separate file
    env_file = os.path.join(os.path.dirname(output_file), 'degen-api.postman_environment.json')
    with open(env_file, 'w') as f:
        json.dump(environment, f, indent=2)
    
    print(f"Postman collection saved to: {output_file}")
    print(f"Postman environment saved to: {env_file}")

if __name__ == "__main__":
    if len(sys.argv) != 3:
        print(f"Usage: {sys.argv[0]} <openapi_file> <output_file>")
        sys.exit(1)
    
    generate_postman_collection(sys.argv[1], sys.argv[2])
