import json
import os
import sys
import uuid
import time
from datetime import datetime

def parse_expiry(expiry_str):
    if not expiry_str:
        return int(time.time()) + 3600
    try:
        # e.g. "2026-09-17T00:54:25.454132+00:00"
        dt = datetime.fromisoformat(expiry_str)
        return int(dt.timestamp())
    except Exception:
        return int(time.time()) + 3600

def main():
    creds_dir = r"C:\Users\Administrator\google2api\creds"
    antigravity_dir = os.path.join(creds_dir, "antigravity")
    state_file = os.path.join(creds_dir, "antigravity_state.json")

    # Target data directory in Antigravity-Tools
    target_data_dir = r"C:\Users\Administrator\Antigravity-Tools\src-tauri\target\release\data"
    target_accounts_dir = os.path.join(target_data_dir, "accounts")
    target_index_file = os.path.join(target_data_dir, "accounts.json")

    os.makedirs(target_accounts_dir, exist_ok=True)

    state_data = {}
    if os.path.exists(state_file):
        with open(state_file, "r", encoding="utf-8") as f:
            try:
                state_data = json.load(f)
            except Exception as e:
                print(f"Warn: failed to load state file: {e}")

    # Load existing accounts index
    existing_index = {"version": "2.0", "accounts": [], "current_account_id": None, "current_target_ide": None}
    if os.path.exists(target_index_file):
        with open(target_index_file, "r", encoding="utf-8") as f:
            try:
                existing_index = json.load(f)
            except Exception as e:
                print(f"Warn: failed to load existing accounts.json: {e}")

    # Map existing accounts by email
    existing_accounts_by_email = {}
    for acc in existing_index.get("accounts", []):
        existing_accounts_by_email[acc.get("email")] = acc

    imported_count = 0
    updated_count = 0
    now = int(time.time())

    # Scan antigravity directory
    for fname in sorted(os.listdir(antigravity_dir)):
        if not fname.endswith(".json"):
            continue

        fpath = os.path.join(antigravity_dir, fname)
        with open(fpath, "r", encoding="utf-8") as f:
            try:
                cred = json.load(f)
            except Exception as e:
                print(f"Error reading {fname}: {e}")
                continue

        email = fname[:-5] # strip .json
        st = state_data.get(fname, {})
        user_name = st.get("user_name") or email.split("@")[0]
        user_email = st.get("user_email") or email

        access_token = cred.get("access_token") or cred.get("token") or ""
        refresh_token = cred.get("refresh_token") or ""
        project_id = cred.get("project_id") or "aicode-consumers"
        expiry_ts = parse_expiry(cred.get("expiry"))
        expires_in = max(0, expiry_ts - now)

        if not refresh_token:
            print(f"Skipping {email}: no refresh token")
            continue

        # Check if already exists in target
        if user_email in existing_accounts_by_email:
            acc_summary = existing_accounts_by_email[user_email]
            acc_id = acc_summary["id"]
            acc_file = os.path.join(target_accounts_dir, f"{acc_id}.json")
            
            # Load existing account details if available
            acc_detail = {}
            if os.path.exists(acc_file):
                with open(acc_file, "r", encoding="utf-8") as af:
                    try:
                        acc_detail = json.load(af)
                    except Exception:
                        pass

            # Update tokens
            acc_detail["id"] = acc_id
            acc_detail["email"] = user_email
            acc_detail["name"] = acc_detail.get("name") or user_name
            if "token" not in acc_detail:
                acc_detail["token"] = {}
            acc_detail["token"]["access_token"] = access_token
            acc_detail["token"]["refresh_token"] = refresh_token
            acc_detail["token"]["expires_in"] = expires_in
            acc_detail["token"]["expiry_timestamp"] = expiry_ts
            acc_detail["token"]["token_type"] = "Bearer"
            acc_detail["token"]["email"] = user_email
            acc_detail["token"]["project_id"] = project_id
            acc_detail["token"]["oauth_client_key"] = "antigravity_enterprise"
            acc_detail["token"]["is_gcp_tos"] = False

            with open(acc_file, "w", encoding="utf-8") as af:
                json.dump(acc_detail, af, indent=2, ensure_ascii=False)

            print(f"[Update] Updated existing account: {user_email} (ID: {acc_id})")
            updated_count += 1
        else:
            acc_id = str(uuid.uuid4())
            new_acc = {
                "id": acc_id,
                "email": user_email,
                "name": user_name,
                "token": {
                    "access_token": access_token,
                    "refresh_token": refresh_token,
                    "expires_in": expires_in,
                    "expiry_timestamp": expiry_ts,
                    "token_type": "Bearer",
                    "email": user_email,
                    "project_id": project_id,
                    "oauth_client_key": "antigravity_enterprise",
                    "is_gcp_tos": False,
                    "id_token": None
                },
                "device_profile": None,
                "device_history": [],
                "quota": None,
                "disabled": False,
                "proxy_disabled": False,
                "created_at": now,
                "last_used": now
            }

            acc_file = os.path.join(target_accounts_dir, f"{acc_id}.json")
            with open(acc_file, "w", encoding="utf-8") as af:
                json.dump(new_acc, af, indent=2, ensure_ascii=False)

            summary = {
                "id": acc_id,
                "email": user_email,
                "name": user_name,
                "disabled": False,
                "proxy_disabled": False,
                "created_at": now,
                "last_used": now
            }
            existing_index["accounts"].append(summary)
            print(f"[Import] Imported new account: {user_email} (ID: {acc_id}, Name: {user_name})")
            imported_count += 1

    if not existing_index.get("current_account_id") and existing_index["accounts"]:
        existing_index["current_account_id"] = existing_index["accounts"][0]["id"]

    # Save index
    with open(target_index_file, "w", encoding="utf-8") as f:
        json.dump(existing_index, f, indent=2, ensure_ascii=False)

    print(f"\nSummary: {imported_count} imported, {updated_count} updated. Total in index: {len(existing_index['accounts'])}")

    # Also export portable backup json for Linux / cross-platform deployment
    export_file = os.path.join(r"C:\Users\Administrator\Antigravity-Tools", "google2api_imported_accounts.json")
    with open(export_file, "w", encoding="utf-8") as f:
        json.dump(existing_index, f, indent=2, ensure_ascii=False)
    print(f"Exported all accounts to: {export_file}")

if __name__ == "__main__":
    main()
