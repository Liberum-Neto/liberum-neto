import time
import matplotlib.pyplot as plt
import numpy as np
import os
import subprocess
import random

CORE_BIN = "./target/release/liberum_core"
CLI_BIN = "./target/release/liberum_cli"
INIT_COUNT = 5
NODE_COUNT = 50
MEASURE_EVERY=3
DOWNLOAD_EVERY=3
DOWNLOAD_PERCENT=50
FILE_NAME = os.path.dirname(os.path.realpath(__file__)) + "/test-file.txt"
FILE_CONTENT = "Hello, World!"
AVG_CLI_TIME_NORMALIZATION=1.6 # ms

NODE_ADDR_PREFIX = "/ip6/::1/udp/"
NODE_ADDR_SUFFIX = "/quic-v1"

subprocess.run(["killall", "liberum_core"])
os.system(CORE_BIN + "$CORE_BIN --daemon  &> /dev/null &")
time.sleep(0.5)

# create nodes
N_NAMES=[]
N_IDS=[]
N_ADDRESSES=[]

N = "test_n0"
N_ADDR = NODE_ADDR_PREFIX + str(52136) + NODE_ADDR_SUFFIX

subprocess.run([CLI_BIN, "-d", "new-node", N, "--id-seed", "0"])
subprocess.run([CLI_BIN, "-d", "config-node", N, "add-external-addr", N_ADDR])

subprocess.run([CLI_BIN, "-d", "start-node", N])

ID = subprocess.run([CLI_BIN, "-d", "get-peer-id", N], stdout=subprocess.PIPE).stdout.decode().strip()
N_NAMES.append(N)
N_IDS.append(ID)
N_ADDRESSES.append(N_ADDR)

# tN=100000
# total_time = 0
# for i in range(0,tN):
#     t0 = time.time()
#     subprocess.run([CLI_BIN, "-d", "get-peer-id", N], stdout=subprocess.PIPE).stdout.decode().strip()
#     total_time += time.time()-t0
# print("Średni czas użycia cli: ", total_time/tN*1000, "ms")
# exit(0)

# create and provide file
with open(FILE_NAME, mode="w") as f:
    f.write(FILE_CONTENT)

RESULTS=[]
FIND_TIMES=[]
for i in range (1, NODE_COUNT) :
    N = "test_n" + str(i)
    N_ADDR = NODE_ADDR_PREFIX + str(i + 52136) + NODE_ADDR_SUFFIX

    subprocess.run([CLI_BIN, "-d", "new-node", N, "--id-seed", str(i)])
    subprocess.run([CLI_BIN, "-d", "config-node", N, "add-external-addr", N_ADDR])
    if i > 0:
        subprocess.run([CLI_BIN, "config-node", N, "add-bootstrap-node", N_IDS[i-1], N_ADDRESSES[i-1]])

    subprocess.run([CLI_BIN, "-d", "start-node", N])

    ID = subprocess.run([CLI_BIN, "-d", "get-peer-id", N], stdout=subprocess.PIPE).stdout.decode().strip()
    N_NAMES.append(N)
    N_IDS.append(ID)
    N_ADDRESSES.append(N_ADDR)
    time.sleep(0.01)
    if i == INIT_COUNT:
        FILE_ID=subprocess.run([CLI_BIN, "publish-file", N_NAMES[0], FILE_NAME], stdout=subprocess.PIPE).stdout.decode().strip()
    if i > INIT_COUNT:
        if i % MEASURE_EVERY == 0:
            results_temp = [i+INIT_COUNT,[]]
            find_tiems_temp = [i+INIT_COUNT, 0]
            measurements=0
            for j in range(1, i):
                if j % DOWNLOAD_EVERY == 0 and random.random()*100 > DOWNLOAD_PERCENT:
                    measurements+=1
                    t0 = time.time()
                    RESULT=subprocess.run([CLI_BIN, "-d", "download-file", N_NAMES[j], FILE_ID], stdout=subprocess.PIPE).stdout.decode().strip()
                    t = time.time()-t0
                    cmp = FILE_CONTENT == RESULT
                    results_temp[1].append((j,cmp))
                    find_tiems_temp[1] += t
                if find_tiems_temp[1] > AVG_CLI_TIME_NORMALIZATION:
                    find_tiems_temp[1] = (find_tiems_temp[1] / measurements) * 1000.0 - AVG_CLI_TIME_NORMALIZATION
            if measurements > 0:
                RESULTS.append(results_temp)
                FIND_TIMES.append(find_tiems_temp)


    # if i == NODE_COUNT//2:
    #     FILE_ID=subprocess.run([CLI_BIN, "publish-file", N_NAMES[0], FILE_NAME], stdout=subprocess.PIPE).stdout.decode().strip()


for x in range(0, len(RESULTS)):
    print(RESULTS[x])
    data = [i[0] for i in RESULTS[x][1] if i[1] == True]
    X = [RESULTS[x][0]] * len(data)
    plt.scatter(X, data, c='b')
plt.title("Udane wyszukania, B=" + str(INIT_COUNT) + ", N=" +  str(NODE_COUNT))
plt.ylabel("Numer węzła")
plt.xlabel("n - ilość węzłów w sieci")
plt.savefig('udane-wyszukania_B='+str(INIT_COUNT)+'_N='+str(NODE_COUNT)+".svg")
plt.show()

failed_counts = []
for x in range(0, len(RESULTS)):
    found = [i[0] for i in RESULTS[x][1] if i[1] == True]
    failed = len(RESULTS[x][1]) - len(found)
    failed_counts.append(failed)
X = [x[0] for x in RESULTS]
print(X)
print(failed_counts)
print(RESULTS)

plt.plot(X, failed_counts, 'b')
plt.title("Liczba nieudanch wyszukań, B=" + str(INIT_COUNT) + ", N=" +  str(NODE_COUNT))
plt.ylabel("Liczba nieudanch wyszukań")
plt.xlabel("n - ilość węzłów w sieci")
plt.savefig('nieudane-wyszukania_B='+str(INIT_COUNT)+'_N='+str(NODE_COUNT)+".svg")
plt.show()

X = [x[0] for x in FIND_TIMES]
Y = [x[1] for x in FIND_TIMES]
plt.plot(X, Y, 'b')
plt.title("Średni czas odnajdywania, B=" + str(INIT_COUNT) + ", N=" +  str(NODE_COUNT))
plt.ylabel("Czas [ms]")
plt.xlabel("n - ilość węzłów w sieci")
plt.savefig('czas-wyszukania_B='+str(INIT_COUNT)+'_N='+str(NODE_COUNT)+".svg")
plt.show()


for i in range(0, NODE_COUNT):
    subprocess.run([CLI_BIN, "stop-node", N_NAMES[i]])

subprocess.run(["killall", "liberum_core"])
