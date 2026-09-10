curl -LsSf https://astral.sh/uv/install.sh | sh
curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.39.3/install.sh | bash
sudo apt install openjdk-21-jdk
sudo apt install openjdk-8-jdk
# sudo update-alternatives --config java
# sudo update-alternatives --config javac
# sudo update-alternatives --config javadoc
curl --proto '=https' --tlsv1.2 https://sh.rustup.rs -sSf | sh

cd /home/agent && source playwright_env/bin/activate && uv pip install playwright