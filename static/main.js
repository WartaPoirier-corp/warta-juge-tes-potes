const socket = new WebSocket('ws://localhost:8008')

const send = msg => socket.send(JSON.stringify(msg))

// State
let game = 'CACA'
let players = [ 'TIBO', 'Analogie' ]
let question = 'Qui est un prout ?'

socket.addEventListener('message', event => {
    const data = JSON.parse(event.data)
    switch (data.tag) {
        case 'RoomCreated':
            game = data.code
            break
        case 'RoomJoin':
            game = data.code
            players = data.players
            m.redraw()
            break
        case 'RoomUpdate':
            players = data.players
            m.redraw()
            break
        case 'NewRound':
        case 'GameStart':
            question = data.question
            m.route.set('/question')
            break
        default:
            console.log('Unknown message', data)
            break
    }
})

const Home = {
    view: () => {
        return m('main', {}, [
            m('section', {}, [
                m('h2', {}, 'Créer une partie'),
                m('p', {}, 'Creéz une partie depuis un ordinateur (ou autre grand écran que tout le monde peut voir). Vous pourrez ensuite la rejoindre avec vos téléphones.'),
                m('a', { className: 'button', onclick: () => {
                    socket.send('CREATE')
                } }, 'Créer une partie')
            ]),
            m('section', {}, [
                m('h2', {}, 'Rejoindre une partie'),
                m('p', {}, 'Entrez le code de la partie qui s\'affiche sur le grand écran.'),
                m('input', { id: 'code' }),
                m('a', { className: 'button', href: '#', onclick: () => {
                    socket.send('JOIN ' + document.getElementById('code').value)
                } }, 'Rejoindre')
            ])
        ])
    }
}

const Lobby = {
    view: () => m('main', {}, [
        m('h2', {}, game),
        m('section', { className: 'lobby' },
            players.map(x => m('div', {}, [
                m('img', { className: 'avatar' }),
                m('p', {}, x),
            ]))
        )
    ])
}

const Question = {
    view: () => m('main', {}, [
        m('h2', {}, question),
        m('section', {}, players.map(x => m('a', { className: 'button' }, x)))
    ])
}

m.route(document.getElementById('app'), '/lobby', {
    '/home': Home,
    '/lobby': Lobby,
    '/question': Question,
})