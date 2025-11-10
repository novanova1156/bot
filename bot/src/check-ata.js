// check-ata.js

const { Connection, PublicKey } = require('@solana/web3.js');
const { getAccount, TOKEN_PROGRAM_ID } = require('@solana/spl-token');

const connection = new Connection('[https://api.devnet.solana.com](https://api.devnet.solana.com)', 'confirmed');

const ATA_ADDRESSES = [
    'ASkpsRKwGKbUmmMrdknqnHVrmxsU8Ws6qeX5iE6AUERK',
    '73weDmPbP1pjZyshM47xiUV6pSadjoZPEtHeczAmNE7P',
    'HkuaPeSMog2jbGMYm7vjPFSaheM69PS1VXoioTp25sxS'
];

async function checkATA() {
    console.log('🔍 Проверка Associated Token Accounts...');

    for (let i = 0; i < ATA_ADDRESSES.length; i++) {
        const ata = new PublicKey(ATA_ADDRESSES[i]);

        try {
            const accountInfo = await connection.getAccountInfo(ata);

            if (accountInfo) {
                const tokenAccount = await getAccount(connection, ata);
                console.log(`✅ ATA ${i + 1}: ${ata.toString()}`);
                console.log(`   Mint: ${tokenAccount.mint.toString()}`);
                console.log(`   Balance: ${tokenAccount.amount.toString()}`);
                console.log(`   Owner: ${tokenAccount.owner.toString()}`);
            } else {
                console.log(`❌ ATA ${i + 1}: ${ata.toString()} - не найден`);
            }
        } catch (error) {
            console.log(`⚠️ ATA ${i + 1}: ${ata.toString()} - ошибка: ${error.message}`);
        }

        console.log('');
    }
}

checkATA().catch(console.error);